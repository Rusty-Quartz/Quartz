use quartz_datapack::{DataPack, VersionFilter};

#[ignore]
#[test]
fn default_datapack_test() {
    let datapack_path = "../run/datapacks/";

    // We use a none filter because we don't want the filter to trigger
    let output = DataPack::read_datapacks(&datapack_path, VersionFilter::None);

    assert!(output.is_ok());

    let output = output.unwrap();

    for pack in output {
        let pack = pack.unwrap();
        let mut path = "../run/out/datapacks/".to_owned();
        path.push_str(pack.name());
        pack.write_datapack(&path).unwrap();
    }
}
