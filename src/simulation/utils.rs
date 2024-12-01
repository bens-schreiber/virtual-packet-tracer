use raylib::{
    color::Color,
    ffi::{self, GuiIconName},
};

pub fn draw_icon(icon: GuiIconName, pos_x: i32, pos_y: i32, pixel_size: i32, color: Color) {
    unsafe {
        ffi::GuiDrawIcon(
            icon as i32,
            pos_x,
            pos_y,
            pixel_size,
            ffi::Color {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            },
        );
    };
}

pub fn rstr_from_string(s: String) -> std::ffi::CString {
    std::ffi::CString::new(s).expect("CString::new failed")
}

pub fn array_to_string(array: &[u8]) -> String {
    let end = array.iter().position(|&c| c == 0).unwrap_or(array.len());
    let slice = &array[..end];
    String::from_utf8_lossy(slice).to_string()
}
