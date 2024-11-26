use std::fs;
use tonic_build;
use walkdir::WalkDir;

fn main() {
    // Указываем директорию с .proto файлами
    let proto_dir = "src/proto";

    // Используем WalkDir для рекурсивного поиска всех .proto файлов
    let proto_files: Vec<String> = WalkDir::new(proto_dir)
        .into_iter()
        .filter_map(|entry| entry.ok()) // Игнорируем ошибки
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "proto")
                .unwrap_or(false)
        }) // Оставляем только .proto файлы
        .map(|entry| entry.path().to_str().unwrap().to_string()) // Преобразуем путь в строку
        .collect();

    let out_dir = "src/xray_api";

    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!("Failed to create directory {:?}: {}", out_dir, e);
        std::process::exit(1);
    }

    // Компилируем все найденные .proto файлы
    tonic_build::configure()
        .build_client(true) // Генерируем код для клиента
        .build_server(false) // Не генерируем код для сервера (если он не нужен)
        .compile_protos(&proto_files, &[proto_dir]) // Указываем все .proto файлы и исходную директорию
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

    println!("cargo:rerun-if-changed={}", proto_dir); // Перегенерация при изменении файлов в директории
}
