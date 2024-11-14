---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
title: Ideas For The Reader
description: Ideas For The Reader
---

The game is visually bare. Here are some ideas on how you could make further changes to enhance it:

-   The tiles could have rounded corners, to look less sharp. Use the [border-radius](/master/docs/slint/reference/elements/rectangle#border-radius)
    property of _[Rectangle](/master/docs/slint/reference/elements/rectangle)_ to achieve that.

-   In real-world memory games, the back of the tiles often have some common graphic. You could add an image with
    the help of another _[Image](/master/docs/slint/reference/elements/image)_
    element. Note that you may have to use _Rectangle_'s _clip property_
    element around it to ensure that the image is clipped away when the curtain effect opens.

