mod app;

use pifile::icons::PifileAssets;
use std::path::PathBuf;

fn main() {
    if std::env::args()
        .nth(1)
        .is_some_and(|arg| arg == "--version" || arg == "-V")
    {
        println!("pifile {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let open_path = std::env::args().nth(1).map(PathBuf::from);

    let app = gpui_kit::application().with_assets(PifileAssets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        app::init(cx);
        cx.activate(true);

        let path = open_path.clone();
        cx.spawn(async move |cx| {
            app::open_window(path, cx).expect("failed to open window");
        })
        .detach();
    });
}
