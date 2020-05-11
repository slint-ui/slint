type Coord = f32;

#[derive(Debug, Default)]
pub struct LayoutData {
    // inputs
    min: Coord,
    max: Coord,
    pref: Coord,
    stretch: f32,

    // outputs
    pos: Coord,
    size: Coord,
}

/// Layout the items within a specified size
///
/// This is quite a simple implementation for now
pub fn layout_items(data: &mut [LayoutData], start_pos: Coord, size: Coord) {
    let (min, _max, perf, mut s) = data.iter().fold((0., 0., 0., 0.), |(min, max, pref, s), it| {
        (min + it.min, max + it.max, pref + it.pref, s + it.stretch)
    });
    if size >= perf {
        // bigger than the prefered size

        // distribute each item its prefered size
        let mut pos = start_pos;
        for it in data.iter_mut() {
            it.size = it.pref;
            it.pos = pos;
            pos += it.size;
        }

        // Allocate the space according to the stretch. Until all space is distributed, or all item
        // have reached their maximum size
        let mut extra_space = size - perf;
        while s > 0. && extra_space > 0. {
            let extra_per_stretch = extra_space / s;
            s = 0.;
            let mut pos = start_pos;
            for it in data.iter_mut() {
                let give = (extra_per_stretch * it.stretch).min(it.max - it.size);
                it.size += give;
                extra_space -= give;
                if give > 0. {
                    s += it.stretch;
                }
                it.pos = pos;
                pos += it.size;
            }
        }
    } else
    /*if size < min*/
    {
        // We have less than the minimum size
        // distribute the difference proportional to the size (TODO: and stretch)
        let ratio = size / min;
        let mut pos = start_pos;
        for it in data {
            it.size = it.min * ratio;
            it.pos = pos;
            pos += it.size;
        }
    }
}

#[test]
fn test_layout_items() {
    let my_items = &mut [
        LayoutData { min: 100., max: 200., pref: 100., stretch: 1., ..Default::default() },
        LayoutData { min: 50., max: 300., pref: 100., stretch: 1., ..Default::default() },
        LayoutData { min: 50., max: 150., pref: 100., stretch: 1., ..Default::default() },
    ];

    layout_items(my_items, 100., 650.);
    assert_eq!(my_items[0].size, 200.);
    assert_eq!(my_items[1].size, 300.);
    assert_eq!(my_items[2].size, 150.);

    layout_items(my_items, 100., 200.);
    assert_eq!(my_items[0].size, 100.);
    assert_eq!(my_items[1].size, 50.);
    assert_eq!(my_items[2].size, 50.);

    layout_items(my_items, 100., 300.);
    assert_eq!(my_items[0].size, 100.);
    assert_eq!(my_items[1].size, 100.);
    assert_eq!(my_items[2].size, 100.);
}
