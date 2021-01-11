use super::CanvasRc;
use sixtyfps_corelib::graphics::FontRequest;
use std::cell::RefCell;

struct LocalFont {
    family: String,
    weight: u16,
    data: &'static [u8],
}

thread_local! {
    // Flat list of "all" fonts. This should be switch back to using fontdb once the new
    // cargo dependency solver is stable and we can depend on fontdb without fs/memmap2
    // for the wasm build.
    static FONTS: RefCell<Vec<LocalFont>> = RefCell::new(Vec::new())
}

fn err_str(message: &'static str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
}

fn load_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    for index in 0..ttf_parser::fonts_in_collection(data).unwrap_or(1) {
        let face = ttf_parser::Face::from_slice(data, index).map_err(|err| Box::new(err))?;

        let family = face
            .names()
            .find(|name| name.name_id() == ttf_parser::name_id::FAMILY && name.is_unicode())
            .ok_or(err_str("Cannot locate family name id in font"))?;

        let family = family.to_string().ok_or(err_str("Empty family name"))?;

        let weight = face.weight().to_number();

        FONTS.with(|fonts| fonts.borrow_mut().push(LocalFont { family, weight, data }));
    }

    Ok(())
}

pub fn register_application_font_from_memory(
    data: &'static [u8],
) -> Result<(), Box<dyn std::error::Error>> {
    maybe_init_fonts();
    load_font_from_memory(data)
}

fn maybe_init_fonts() {
    if FONTS.with(|fonts| fonts.borrow().is_empty()) {
        load_font_from_memory(include_bytes!("fonts/Roboto-Regular.ttf")).unwrap();
        load_font_from_memory(include_bytes!("fonts/Roboto-Bold.ttf")).unwrap();
    }
}

fn find_family_match<'a>(
    fonts: &'a Vec<LocalFont>,
    requested_family_name: &'a str,
) -> impl Iterator<Item = &'a LocalFont> + Clone + 'a {
    fonts.iter().flat_map(move |local_font| {
        if requested_family_name == local_font.family {
            Some(local_font)
        } else {
            None
        }
    })
}

pub(crate) fn try_load_app_font(
    canvas: &CanvasRc,
    request: &FontRequest,
) -> Option<femtovg::FontId> {
    maybe_init_fonts();

    let requested_family = if request.family.is_empty() { "Roboto" } else { &request.family };
    let requested_weight = request.weight.unwrap();

    FONTS.with(|fonts| {
        let fonts = fonts.borrow();

        let family_matches = find_family_match(&*fonts, requested_family);

        let font_match = family_matches
            .clone()
            .filter(|family_match| requested_weight == family_match.weight.into())
            .chain(family_matches)
            .next()
            .unwrap_or_else(|| fonts.first().unwrap());

        canvas.borrow_mut().add_font_mem(font_match.data).ok()
    })
}

pub(crate) fn load_system_font(_canvas: &CanvasRc, _request: &FontRequest) -> femtovg::FontId {
    unreachable!()
}
