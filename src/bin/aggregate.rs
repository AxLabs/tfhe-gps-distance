use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Command;

fn find_bin_path(bin_name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let dir = exe.parent().expect("parent dir of exe");
    dir.join(bin_name)
}

fn run_approach(bin: &str, args: &[String]) -> Result<String, String> {
    let bin_path = find_bin_path(bin);
    let mut cmd = Command::new(bin_path);
    if !args.is_empty() {
        cmd.args(args);
    }
    let out = cmd.output().map_err(|e| format!("failed to run {}: {}", bin, e))?;
    if !out.status.success() {
        return Err(format!(
            "{} exited with status {}\nstdout:\n{}\nstderr:\n{}",
            bin,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_timings(stdout: &str) -> HashMap<String, f64> {
    let mut map = HashMap::new();
    for line in stdout.lines() {
        if let Some((label, rest)) = line.split_once(" = ") {
            // expect seconds suffix like "0.123456 s"
            if let Some(val_str) = rest.strip_suffix(" s") {
                if let Ok(val) = val_str.trim().parse::<f64>() {
                    map.insert(label.trim().to_string(), val);
                }
            }
        }
    }
    map
}

fn format_table(rows: &[(String, Option<f64>, Option<f64>)]) -> String {
    let mut label_w = "Step".len();
    let mut a1_w = "Approach1 (s)".len();
    let mut a2_w = "Approach2 (s)".len();
    for (l, v1, v2) in rows.iter() {
        if l.len() > label_w { label_w = l.len(); }
        let a1s = v1.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".to_string());
        if a1s.len() > a1_w { a1_w = a1s.len(); }
        let a2s = v2.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".to_string());
        if a2s.len() > a2_w { a2_w = a2s.len(); }
    }
    let header = format!(
        "{:<label_w$} | {:>a1_w$} | {:>a2_w$}",
        "Step", "Approach1 (s)", "Approach2 (s)",
        label_w = label_w, a1_w = a1_w, a2_w = a2_w
    );
    let sep = format!("{}-+-{}-+-{}",
        "-".repeat(label_w), "-".repeat(a1_w), "-".repeat(a2_w));
    let mut lines = vec![header, sep];
    for (l, v1, v2) in rows.iter() {
        let a1s = v1.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".to_string());
        let a2s = v2.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "{:<label_w$} | {:>a1_w$} | {:>a2_w$}",
            l, a1s, a2s, label_w = label_w, a1_w = a1_w, a2_w = a2_w
        ));
    }
    lines.join("\n")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pass-through args: [name1 lat1 lon1 name2 lat2 lon2 name3 lat3 lon3]
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(args.is_empty() || args.len() == 9) {
        eprintln!("Expected either 0 args or 9 args: name1 lat1 lon1 name2 lat2 lon2 name3 lat3 lon3");
        std::process::exit(2);
    }

    println!("Running approach1...");
    let out1 = run_approach("approach1", if args.is_empty() { &[] } else { &args })?;
    println!("Running approach2...");
    let out2 = run_approach("approach2", if args.is_empty() { &[] } else { &args })?;

    let map1 = parse_timings(&out1);
    let map2 = parse_timings(&out2);

    // Build a preferred ordering; missing labels will be added later
    let mut ordered: Vec<String> = vec![
        // Client prep
        "client:step1:precompute+encrypt:Basel".to_string(),
        "client:step1:precompute+encrypt:Lugano".to_string(),
        "client:step1:precompute+encrypt:Zurich".to_string(),
        // Server compute pairs (names depend on inputs; include common defaults)
        "server:step3:compute_a_Basel-Zurich".to_string(),
        "server:step3:compute_a_Lugano-Zurich".to_string(),
        // Shared internals
        "server:step2:compute_deltas".to_string(),
        "server:step3:poly_sin2_half".to_string(),
        "server:step3:combine_a".to_string(),
        // Approach1-only
        "server:step4:poly_arcsin_sqrt".to_string(),
        "server:step5:multiply_radius".to_string(),
        // Final compare
        "server:final:compare".to_string(),
        // Client finalize
        "CLIENT: TOTAL".to_string(),
        "SERVER: TOTAL".to_string(),
        "CLIENT: decrypt compare bit".to_string(),
    ];

    // Add any other labels encountered to the end
    let mut all_labels: BTreeSet<String> = BTreeSet::new();
    all_labels.extend(map1.keys().cloned());
    all_labels.extend(map2.keys().cloned());
    for l in all_labels {
        if !ordered.iter().any(|x| x == &l) {
            ordered.push(l);
        }
    }

    let mut rows: Vec<(String, Option<f64>, Option<f64>)> = Vec::new();
    for l in ordered.iter() {
        let v1 = map1.get(l).copied();
        let v2 = map2.get(l).copied();
        if v1.is_some() || v2.is_some() {
            rows.push((l.clone(), v1, v2));
        }
    }

    println!("\nAggregated timings (seconds):\n");
    println!("{}", format_table(&rows));

    Ok(())
}


