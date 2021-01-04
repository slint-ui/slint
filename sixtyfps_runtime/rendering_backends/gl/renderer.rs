use femtovg::*;
use sixtyfps_corelib::{graphics::Rect, SharedVector};
use sixtyfps_corelib::{
    items::{Item, ItemRenderer},
    Resource,
};

struct FemtoRenderer {
    canvas: Canvas<femtovg::renderer::OpenGl>,
    clip_rects: SharedVector<Rect>,
}

fn rect_to_path(r: Rect) -> Path {
    let mut path = Path::new();
    path.rect(r.min_x(), r.min_y(), r.width(), r.height());
    path
}

impl FemtoRenderer {
    fn load_image(&mut self, source: Resource) -> Option<ImageId> {
        match source {
            Resource::None => None,
            Resource::AbsoluteFilePath(path) => self
                .canvas
                .load_image_file(std::path::Path::new(&path.as_str()), femtovg::ImageFlags::empty())
                .ok(),
            Resource::EmbeddedData(data) => {
                self.canvas.load_image_mem(data.as_slice(), femtovg::ImageFlags::empty()).ok()
            }
            Resource::EmbeddedRgbaImage { width, height, data } => todo!(),
        }
    }
}

impl ItemRenderer for FemtoRenderer {
    fn draw_rectangle(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::Rectangle>,
    ) {
        // TODO: cache path in item to avoid re-tesselation
        let mut path = rect_to_path(rect.geometry());
        let paint = Paint::color(
            sixtyfps_corelib::items::Rectangle::FIELD_OFFSETS.color.apply_pin(rect).get().into(),
        );
        self.canvas.save_with(|canvas| {
            canvas.translate(pos.x, pos.y);
            canvas.fill_path(&mut path, paint)
        })
    }

    fn draw_border_rectangle(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::BorderRectangle>,
    ) {
        // TODO: cache path in item to avoid re-tesselation
        let mut path = Path::new();
        path.rounded_rect(
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS.x.apply_pin(rect).get(),
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS.y.apply_pin(rect).get(),
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS.width.apply_pin(rect).get(),
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS.height.apply_pin(rect).get(),
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS
                .border_radius
                .apply_pin(rect)
                .get(),
        );
        let fill_paint = Paint::color(
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS
                .color
                .apply_pin(rect)
                .get()
                .into(),
        );
        let mut border_paint = Paint::color(
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS
                .border_color
                .apply_pin(rect)
                .get()
                .into(),
        );
        border_paint.set_line_width(
            sixtyfps_corelib::items::BorderRectangle::FIELD_OFFSETS
                .border_width
                .apply_pin(rect)
                .get(),
        );
        self.canvas.save_with(|canvas| {
            canvas.translate(pos.x, pos.y);
            canvas.fill_path(&mut path, fill_paint)
        })
    }

    fn draw_image(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        image: std::pin::Pin<&sixtyfps_corelib::items::Image>,
    ) {
        // TODO: cache
        let image_id = self
            .load_image(sixtyfps_corelib::items::Image::FIELD_OFFSETS.source.apply_pin(image).get())
            .unwrap();

        let info = self.canvas.image_info(image_id).unwrap();

        let (image_width, image_height) = (info.width() as f32, info.height() as f32);
        let (source_width, source_height) = (image_width, image_height);
        let fill_paint =
            femtovg::Paint::image(image_id, 0., 0., source_width, source_height, 0.0, 1.0);

        let mut path = Path::new();
        path.rect(pos.x, pos.y, image_width, image_height);

        self.canvas.save_with(|canvas| {
            let scaled_width =
                sixtyfps_corelib::items::Image::FIELD_OFFSETS.width.apply_pin(image).get();
            let scaled_height =
                sixtyfps_corelib::items::Image::FIELD_OFFSETS.height.apply_pin(image).get();
            if scaled_width > 0. && scaled_height > 0. {
                canvas.scale(scaled_width / image_width, scaled_height / image_height);
            }

            canvas.fill_path(&mut path, fill_paint);
        })
    }

    fn draw_clipped_image(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        clipped_image: std::pin::Pin<&sixtyfps_corelib::items::ClippedImage>,
    ) {
        // TODO: cache
        let image_id = self
            .load_image(
                sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS
                    .source
                    .apply_pin(clipped_image)
                    .get(),
            )
            .unwrap();

        let info = self.canvas.image_info(image_id).unwrap();

        let source_clip_rect = Rect::new(
            [
                sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS
                    .source_clip_x
                    .apply_pin(clipped_image)
                    .get(),
                sixtyfps_corelib::items::ClippedImage::FIELD_OFFSETS
                    .source_clip_y
                    .apply_pin(clipped_image)
                    .get(),
            ]
            .into(),
            [0., 0.].into(),
        );

        let (image_width, image_height) = (info.width() as f32, info.height() as f32);
        let (source_width, source_height) = (image_width, image_height);
        let fill_paint =
            femtovg::Paint::image(image_id, 0., 0., source_width, source_height, 0.0, 1.0);

        let mut path = Path::new();
        path.rect(pos.x, pos.y, image_width, image_height);

        self.canvas.save_with(|canvas| {
            let scaled_width =
                sixtyfps_corelib::items::Image::FIELD_OFFSETS.width.apply_pin(image).get();
            let scaled_height =
                sixtyfps_corelib::items::Image::FIELD_OFFSETS.height.apply_pin(image).get();
            if scaled_width > 0. && scaled_height > 0. {
                canvas.scale(scaled_width / image_width, scaled_height / image_height);
            }

            canvas.fill_path(&mut path, fill_paint);
        })
    }

    fn draw_text(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::Text>,
    ) {
        todo!()
    }

    fn draw_text_input(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        rect: std::pin::Pin<&sixtyfps_corelib::items::TextInput>,
    ) {
        todo!()
    }

    fn draw_path(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        path: std::pin::Pin<&sixtyfps_corelib::items::Path>,
    ) {
        todo!()
    }

    fn combine_clip(
        &mut self,
        pos: sixtyfps_corelib::graphics::Point,
        clip: &std::pin::Pin<&sixtyfps_corelib::items::Clip>,
    ) {
        let clip_rect = clip.geometry().translate([pos.x, pos.y].into());
        self.canvas.intersect_scissor(
            clip_rect.min_x(),
            clip_rect.min_y(),
            clip_rect.width(),
            clip_rect.height(),
        );
        self.clip_rects.push(clip_rect);
    }

    fn clip_rects(&self) -> SharedVector<sixtyfps_corelib::graphics::Rect> {
        self.clip_rects.clone()
    }

    fn reset_clip(&mut self, rects: SharedVector<sixtyfps_corelib::graphics::Rect>) {
        self.clip_rects = rects;
        // ### Only do this if rects were really changed
        self.canvas.reset_scissor();
        for rect in self.clip_rects.as_slice() {
            self.canvas.intersect_scissor(rect.min_x(), rect.min_y(), rect.width(), rect.height())
        }
    }
}
