extern crate renderer;

fn get_test_file(file_name: &str) -> String {
    let mut test_osm_path = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_osm_path.push("tests");
    test_osm_path.push(file_name);

    test_osm_path.to_str().unwrap().to_string()
}

#[test]
fn test_geodata_reader() {
    renderer::geodata::importer::import(
        &get_test_file("nano_moscow.osm"),
        &get_test_file("nano_moscow.bin")
    ).unwrap();
}
