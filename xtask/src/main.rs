//! Outils de développement wallos-rs.
//!
//! Commandes :
//! - `cargo xtask trace` : matrice de traçabilité + codes TRC
//! - `cargo xtask trace --e2e` : couverture E2E
//! - `cargo xtask openapi` : régénère api/openapi.json
//! - `cargo xtask openapi --check` : drift gate
//! - `cargo xtask api-coverage` : 100% des operation_id testés
//! - `cargo xtask authz-coverage` : 3 tests d'autorisation par operation_id
//! - `cargo xtask lint-money` : interdiction des flottants monétaires
//! - `cargo xtask lint-clock` : interdiction des accès à l'horloge dans `core` (REQ-STA-008)

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Matrice de traçabilité.
    Trace {
        /// Inclure la couverture E2E.
        #[arg(long)]
        e2e: bool,
        /// Régénère les artefacts dérivés (badge shields.io + matrice Markdown).
        #[arg(long)]
        write: bool,
    },
    /// Génère ou vérifie l'OpenAPI.
    Openapi {
        /// Vérifie le drift sans modifier.
        #[arg(long)]
        check: bool,
    },
    /// Vérifie la couverture des opérations API.
    ApiCoverage,
    /// Vérifie la couverture d'autorisation.
    AuthzCoverage,
    /// Interdit les flottants dans les montants.
    LintMoney,
    /// Interdit les accès directs à l'horloge système dans `core`.
    LintClock,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let cli = Cli::parse();
    let root = workspace_root();

    let code = match cli.command {
        Command::Trace { e2e, write } => trace::run(&root, e2e, write),
        Command::Openapi { check } => openapi::run(&root, check),
        Command::ApiCoverage => api_coverage::run(&root),
        Command::AuthzCoverage => authz_coverage::run(&root),
        Command::LintMoney => lint_money::run(&root),
        Command::LintClock => lint_clock::run(&root),
    };
    std::process::exit(code);
}

mod trace {
    use std::collections::HashMap;
    use std::path::Path;

    use yaml_rust2::YamlLoader;

    pub fn run(root: &Path, _e2e: bool, write: bool) -> i32 {
        let lock_path = root.join("spec/requirements.lock.yaml");
        let lock_content = match std::fs::read_to_string(&lock_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("TRC-07: cannot read {:?}: {}", lock_path, e);
                return 1;
            }
        };
        let lock_docs = match YamlLoader::load_from_str(&lock_content) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("TRC-07: cannot parse {:?}: {}", lock_path, e);
                return 1;
            }
        };
        let lock_doc = &lock_docs[0];
        let reqs = match lock_doc["requirements"].as_vec() {
            Some(v) => v,
            None => {
                eprintln!("TRC-07: requirements array missing in {:?}", lock_path);
                return 1;
            }
        };

        let src_dir = root.join("spec/requirements");
        let mut source_ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md")
                    && let Ok(content) = std::fs::read_to_string(&path)
                {
                    for line in content.lines() {
                        if let Some(id) = line.trim().strip_prefix("id: ") {
                            source_ids.push(id.to_string());
                        }
                    }
                }
            }
        }
        let source_ids: std::collections::HashSet<_> = source_ids.into_iter().collect();
        let lock_ids: std::collections::HashSet<_> = reqs
            .iter()
            .filter_map(|r| r["id"].as_str().map(String::from))
            .collect();

        let mut exit = 0;

        let mut source_missing_in_lock: Vec<_> =
            source_ids.difference(&lock_ids).cloned().collect();
        let mut lock_missing_in_source: Vec<_> =
            lock_ids.difference(&source_ids).cloned().collect();
        source_missing_in_lock.sort();
        lock_missing_in_source.sort();
        if !source_missing_in_lock.is_empty() || !lock_missing_in_source.is_empty() {
            eprintln!("TRC-07: requirements.lock.yaml is out of sync with spec/requirements/");
            if !source_missing_in_lock.is_empty() {
                eprintln!("  in source but not lock: {:?}", source_missing_in_lock);
            }
            if !lock_missing_in_source.is_empty() {
                eprintln!("  in lock but not source: {:?}", lock_missing_in_source);
            }
            exit = 1;
        }

        let mut seen = std::collections::HashSet::new();
        let mut dupes = Vec::new();
        for id in lock_ids.iter() {
            if !seen.insert(id) {
                dupes.push(id.clone());
            }
        }
        if !dupes.is_empty() {
            eprintln!("TRC-07: duplicate IDs in lock: {:?}", dupes);
            exit = 1;
        }

        let mut accepted_without_impl = Vec::new();
        let mut accepted_without_test = Vec::new();
        for req in reqs {
            let status = req["status"].as_str().unwrap_or("draft");
            if status != "accepted" {
                continue;
            }
            let id = req["id"].as_str().unwrap_or("UNKNOWN");
            let has_impl = has_annotation(root, id, "requirement");
            let has_test = has_annotation(root, id, "verifies");
            let e2e = req["e2e"].as_str().unwrap_or("optional");
            let has_e2e = if e2e == "required" {
                has_tag(root, id)
            } else {
                true
            };
            if !has_impl {
                accepted_without_impl.push(id);
            }
            if !has_test {
                accepted_without_test.push(id);
            }
            if !has_e2e {
                eprintln!("TRC-03: {id} is accepted and e2e:required but has no Playwright tag");
                exit = 1;
            }
        }
        if !accepted_without_impl.is_empty() {
            eprintln!(
                "TRC-01: accepted requirements without implementation: {:?}",
                accepted_without_impl
            );
            exit = 1;
        }
        if !accepted_without_test.is_empty() {
            eprintln!(
                "TRC-02: accepted requirements without tests: {:?}",
                accepted_without_test
            );
            exit = 1;
        }

        // Fichiers exemptés de TRC-06 (code sans logique annotable : racines de module, types
        // d'erreur, crates stub). Chaque entrée est justifiée dans le fichier (ADR 0021).
        let trace_exclusions = load_trace_exclusions(root);
        for entry in walkdir::WalkDir::new(root.join("crates"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if trace_exclusions.contains(&rel) {
                continue;
            }
            let content = std::fs::read_to_string(path).unwrap_or_default();
            if !content.contains("#[requirement(") && !content.contains("#[verifies(") {
                eprintln!(
                    "TRC-06: production file without requirement annotation: {}",
                    path.display()
                );
                exit = 1;
            }
        }

        let mut by_status: HashMap<&str, usize> = HashMap::new();
        for req in reqs {
            let status = req["status"].as_str().unwrap_or("draft");
            *by_status.entry(status).or_insert(0) += 1;
        }
        // Ordre déterministe pour l'affichage ET les artefacts dérivés (le badge et la matrice
        // doivent être stables entre deux générations, sinon la porte de drift déclenche à tort).
        const STATUS_ORDER: [&str; 4] = ["verified", "implemented", "accepted", "draft"];
        let mut ordered: Vec<(&str, usize)> = Vec::new();
        for s in STATUS_ORDER {
            if let Some(&c) = by_status.get(s) {
                ordered.push((s, c));
            }
        }
        // Statuts hors nomenclature connue (robustesse) : ajoutés à la fin, triés.
        let mut extra: Vec<(&str, usize)> = by_status
            .iter()
            .filter(|(s, _)| !STATUS_ORDER.contains(*s))
            .map(|(s, c)| (*s, *c))
            .collect();
        extra.sort();
        ordered.extend(extra);

        println!("=== Traceability summary ===");
        println!("Total requirements: {}", reqs.len());
        for (status, count) in &ordered {
            println!("  {status}: {count}");
        }

        if write && let Err(e) = write_artifacts(root, reqs, &ordered) {
            eprintln!("TRC-08: cannot write traceability artifacts: {e}");
            exit = 1;
        }

        if exit == 0 {
            println!("Traceability OK.");
        }
        exit
    }

    /// Régénère les deux artefacts dérivés du lock de traçabilité :
    /// - `spec/trace-badge.json` : badge shields.io au schéma « endpoint » (rendu automatique) ;
    /// - `spec/TRACEABILITY.md` : matrice lisible, groupée par domaine.
    ///
    /// Tout est dérivé de `spec/requirements.lock.yaml`, déjà validé comme synchrone avec les sources
    /// par `run`. Aucun flottant (couleur calculée en pourcentage entier).
    fn write_artifacts(
        root: &Path,
        reqs: &[yaml_rust2::Yaml],
        ordered: &[(&str, usize)],
    ) -> std::io::Result<()> {
        let total: usize = ordered.iter().map(|(_, c)| c).sum();
        let verified = ordered
            .iter()
            .find(|(s, _)| *s == "verified")
            .map_or(0, |(_, c)| *c);

        // --- Badge shields.io (schéma « endpoint ») ---
        let message = ordered
            .iter()
            .map(|(s, c)| format!("{c} {s}"))
            .collect::<Vec<_>>()
            .join(" · ");
        // `checked_div` couvre le cas `total == 0` (aucune exigence) sans branche explicite.
        let pct = (verified * 100).checked_div(total).unwrap_or(0);
        let color = match pct {
            100 => "brightgreen",
            75..=99 => "green",
            50..=74 => "yellowgreen",
            25..=49 => "yellow",
            _ => "orange",
        };
        // `{message:?}` produit une chaîne JSON échappée (guillemets/backslash) sans travail manuel.
        let badge = format!(
            "{{\n  \"schemaVersion\": 1,\n  \"label\": \"exigences\",\n  \"message\": {message:?},\n  \"color\": \"{color}\"\n}}\n"
        );
        std::fs::write(root.join("spec/trace-badge.json"), badge)?;

        // --- Matrice Markdown groupée par domaine ---
        let mut by_domain: std::collections::BTreeMap<String, Vec<&yaml_rust2::Yaml>> =
            std::collections::BTreeMap::new();
        for req in reqs {
            let domain = req["domain"]
                .as_str()
                .unwrap_or("(sans domaine)")
                .to_string();
            by_domain.entry(domain).or_default().push(req);
        }

        let mut md = String::new();
        md.push_str(
            "<!-- GÉNÉRÉ par `cargo xtask trace --write` — NE PAS ÉDITER À LA MAIN. -->\n\n",
        );
        md.push_str("# Matrice de traçabilité des exigences\n\n");
        md.push_str(&format!("Total : **{total}** exigences — "));
        md.push_str(&message);
        md.push_str(".\n\n");
        md.push_str(
            "Source de vérité : [`spec/requirements.lock.yaml`](requirements.lock.yaml) et \
             [`spec/requirements/`](requirements/). Régénérer avec `cargo xtask trace --write`.\n\n",
        );
        for (domain, items) in &by_domain {
            md.push_str(&format!("## {domain}\n\n"));
            md.push_str("| ID | Titre | Criticité | Statut |\n");
            md.push_str("|----|-------|-----------|--------|\n");
            for req in items {
                let id = req["id"].as_str().unwrap_or("?");
                let title = req["title"].as_str().unwrap_or("").replace('|', "\\|");
                let crit = req["criticality"].as_str().unwrap_or("-");
                let status = req["status"].as_str().unwrap_or("draft");
                let rendered = match status {
                    "verified" => "✅ verified",
                    "implemented" => "🟡 implemented",
                    "accepted" => "🔵 accepted",
                    "draft" => "⚪ draft",
                    other => other,
                };
                md.push_str(&format!("| `{id}` | {title} | {crit} | {rendered} |\n"));
            }
            md.push('\n');
        }
        std::fs::write(root.join("spec/TRACEABILITY.md"), md)?;
        Ok(())
    }

    fn has_annotation(root: &Path, id: &str, kind: &str) -> bool {
        let pattern = format!("#[{kind}(\"{id}\")]");
        let pattern2 = format!("#[{kind}({id})]");
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
            if content.contains(&pattern) || content.contains(&pattern2) {
                return true;
            }
        }
        false
    }

    fn has_tag(root: &Path, id: &str) -> bool {
        let pattern = format!("@{id}");
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("ts"))
        {
            let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
            if content.contains(&pattern) {
                return true;
            }
        }
        false
    }

    /// Charge les chemins exemptés de TRC-06 depuis `xtask/trace-exclusions.toml`.
    ///
    /// Parseur minimal (pas de dépendance TOML) : chaque chemin `"…/x.rs"` entre guillemets est
    /// collecté ; les commentaires `#` (y compris en fin de ligne) sont ignorés. Voir ADR 0021.
    fn load_trace_exclusions(root: &Path) -> std::collections::HashSet<String> {
        let content =
            std::fs::read_to_string(root.join("xtask/trace-exclusions.toml")).unwrap_or_default();
        let mut set = std::collections::HashSet::new();
        for line in content.lines() {
            let code = line.split('#').next().unwrap_or("");
            let mut rest = code;
            while let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                let Some(end) = after.find('"') else { break };
                let candidate = &after[..end];
                if candidate.ends_with(".rs") {
                    set.insert(candidate.to_string());
                }
                rest = &after[end + 1..];
            }
        }
        set
    }
}

mod openapi {
    use std::path::Path;

    use utoipa::OpenApi;

    pub fn run(root: &Path, check: bool) -> i32 {
        let path = root.join("api/openapi.json");
        let generated = generate_openapi();
        if check {
            let existing = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("R8: cannot read {:?}: {}", path, e);
                    return 1;
                }
            };
            let existing_norm = normalize_json(&existing);
            let generated_norm = normalize_json(&generated);
            if existing_norm != generated_norm {
                eprintln!("R8: OpenAPI drift detected. Regenerate with `cargo xtask openapi`.");
                return 1;
            }
            println!("OpenAPI drift check OK.");
            0
        } else {
            if let Err(e) = std::fs::write(&path, generated) {
                eprintln!("Failed to write {:?}: {}", path, e);
                return 1;
            }
            println!("OpenAPI generated at {:?}.", path);
            0
        }
    }

    fn generate_openapi() -> String {
        let doc = wallos_server::ApiDoc::openapi();
        doc.to_json().expect("utoipa openapi serialization")
    }

    fn normalize_json(s: &str) -> String {
        let v: serde_json::Value = match serde_json::from_str(s) {
            Ok(v) => v,
            Err(_) => return s.to_string(),
        };
        serde_json::to_string(&v).expect("valid json")
    }
}

mod api_coverage {
    use std::path::Path;

    pub fn run(root: &Path) -> i32 {
        let openapi_path = root.join("api/openapi.json");
        let content = match std::fs::read_to_string(&openapi_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Cannot read {:?}: {}", openapi_path, e);
                return 1;
            }
        };
        let doc: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Cannot parse OpenAPI: {}", e);
                return 1;
            }
        };
        let ops = operation_ids(&doc);
        if ops.is_empty() {
            println!("API coverage: 0 operation_ids, 0 tests (100% of 0).");
            return 0;
        }
        let tests = match tests_dir(root) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Cannot read tests: {}", e);
                return 1;
            }
        };
        fn snake(s: &str) -> String {
            let mut out = String::with_capacity(s.len());
            let mut prev_lower = false;
            for ch in s.chars() {
                if ch.is_uppercase() {
                    if prev_lower {
                        out.push('_');
                    }
                    out.push(ch.to_ascii_lowercase());
                    prev_lower = false;
                } else {
                    out.push(ch);
                    prev_lower = ch.is_ascii_lowercase();
                }
            }
            out
        }
        let mut uncovered = Vec::new();
        for op in &ops {
            if !tests.contains(op) && !tests.contains(&snake(op)) {
                uncovered.push(op.clone());
            }
        }
        if uncovered.is_empty() {
            println!("API coverage: 100% ({} operations).", ops.len());
            0
        } else {
            eprintln!(
                "API coverage failure: uncovered operation_ids: {:?}",
                uncovered
            );
            1
        }
    }

    pub fn operation_ids(doc: &serde_json::Value) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some(paths) = doc["paths"].as_object() {
            for (_path, methods) in paths {
                if let Some(methods) = methods.as_object() {
                    for (_method, op) in methods {
                        if let Some(id) = op["operationId"].as_str() {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
        }
        ids
    }

    pub fn tests_dir(root: &Path) -> Result<String, std::io::Error> {
        let mut content = String::new();
        for entry in walkdir::WalkDir::new(root.join("crates/server/tests"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            content.push_str(&std::fs::read_to_string(entry.path())?);
        }
        Ok(content)
    }
}

mod authz_coverage {
    use std::path::Path;

    use crate::api_coverage::{operation_ids, tests_dir};

    pub fn run(root: &Path) -> i32 {
        let openapi_path = root.join("api/openapi.json");
        let content = match std::fs::read_to_string(&openapi_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Cannot read {:?}: {}", openapi_path, e);
                return 1;
            }
        };
        let doc: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Cannot parse OpenAPI: {}", e);
                return 1;
            }
        };
        let ops = operation_ids(&doc);
        if ops.is_empty() {
            println!("Authz coverage: 0 operations, 0 tests (100% of 0).");
            return 0;
        }
        let tests = match tests_dir(root) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Cannot read tests: {}", e);
                return 1;
            }
        };
        let mut missing = Vec::new();
        for op in &ops {
            fn snake(s: &str) -> String {
                let mut out = String::with_capacity(s.len());
                let mut prev_lower = false;
                for ch in s.chars() {
                    if ch.is_uppercase() {
                        if prev_lower {
                            out.push('_');
                        }
                        out.push(ch.to_ascii_lowercase());
                        prev_lower = false;
                    } else {
                        out.push(ch);
                        prev_lower = ch.is_ascii_lowercase();
                    }
                }
                out
            }
            let op_snake = snake(op);
            let owner = format!("authz_owner_{op_snake}");
            let other = format!("authz_other_{op_snake}");
            let anon = format!("authz_anon_{op_snake}");
            if !tests.contains(&owner) || !tests.contains(&other) || !tests.contains(&anon) {
                missing.push(op.clone());
            }
        }
        if missing.is_empty() {
            println!("Authz coverage: 100% ({} operations).", ops.len());
            0
        } else {
            eprintln!(
                "Authz coverage failure: missing 3-authz tests for: {:?}",
                missing
            );
            1
        }
    }
}

mod lint_money {
    use regex::Regex;
    use std::path::Path;
    use walkdir::WalkDir;

    pub fn run(root: &Path) -> i32 {
        let re = Regex::new(r"\b(f32|f64)\b").expect("valid regex");
        let crates = [
            "core", "proto", "storage", "server", "notifier", "client", "desktop",
        ];
        let mut violations = vec![];
        for crate_name in crates {
            for entry in WalkDir::new(root.join("crates").join(crate_name))
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
            {
                let path = entry.path();
                let content = std::fs::read_to_string(path).unwrap_or_default();
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        violations.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                    }
                }
            }
        }

        if violations.is_empty() {
            println!("No float usage in production crates.");
            0
        } else {
            eprintln!("Float types found in production crates:");
            for v in violations {
                eprintln!("  {v}");
            }
            1
        }
    }
}

/// Interdit les accès directs à l'horloge système dans le domaine pur `core`.
///
/// REQ-STA-008 : un agrégat dépendant de l'horloge n'est pas reproductible. La date de
/// référence doit toujours être un paramètre explicite (`core::stats::AsOf`).
mod lint_clock {
    use regex::Regex;
    use std::path::Path;
    use walkdir::WalkDir;

    pub fn run(root: &Path) -> i32 {
        let re = Regex::new(r"\b(Utc|Local|SystemTime|Instant|OffsetDateTime)::now\b|\bnow_utc\b")
            .expect("valid regex");
        let mut violations = vec![];
        for entry in WalkDir::new(root.join("crates").join("core"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path).unwrap_or_default();
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    violations.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }

        if violations.is_empty() {
            println!("No system-clock access in core.");
            0
        } else {
            eprintln!("System-clock access found in core (REQ-STA-008):");
            for v in violations {
                eprintln!("  {v}");
            }
            1
        }
    }
}
