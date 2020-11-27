/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

import plaster_font_url from "../plaster-font/Plaster-Regular.ttf";

let plaster_font_face = new FontFace("Plaster", `url(${plaster_font_url})`);
plaster_font_face.load().then(() => {
    document.fonts.add(plaster_font_face);
    import('./pkg/index.js')
});
