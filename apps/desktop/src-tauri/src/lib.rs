use serde::Serialize;

#[derive(Serialize)]
struct PatternResponse {
    vertex_count: usize,
    edge_count: usize,
}

#[tauri::command]
fn generate_benchmark_pattern(edge_count: usize) -> PatternResponse {
    let pattern = ori_core::benchmark_pattern(edge_count.min(100_000));
    PatternResponse {
        vertex_count: pattern.vertices.len(),
        edge_count: pattern.edges.len(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![generate_benchmark_pattern])
        .run(tauri::generate_context!())
        .expect("failed to run ORIGAMI2 desktop application");
}
