use once_cell::sync::Lazy;
use std::sync::Arc;

pub static HOME_DIR: Lazy<Arc<str>> = Lazy::new(|| {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USER").map(|user| format!("/home/{}", user)))
        .map(Arc::from)
        .expect("HOME or USER environment variable must be set")
});

pub static CONFIG_DIR: Lazy<Arc<str>> = Lazy::new(|| {
    std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| Ok::<String, ()>(format!("{}/.config", HOME_DIR.as_ref())))
        .map(Arc::from)
        .unwrap() // expect in HOME_DIR will be triggered first
});

pub static VRC_DIR: Lazy<Arc<str>> = Lazy::new(|| {
    Arc::from(
        format!(
            "{}/.local/share/Steam/steamapps/compatdata/438100/pfx/drive_c/users/steamuser/AppData/LocalLow/VRChat/VRChat/", 
            HOME_DIR.as_ref()
        )
    )
});

pub static OSC_DIR: Lazy<Arc<str>> = Lazy::new(|| Arc::from(format!("{}/OSC/", VRC_DIR.as_ref())));
