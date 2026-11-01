// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore splitn

//! The airports to fly between and the cities to label, from `data/`.

use glam::Vec3;

use crate::camera::surface;

pub struct Airport {
    pub code: String,
    pub city: String,
    pub country: String,
    pub name: String,
    pub position: Vec3,
    /// The code, city, country, and name in lowercase without accents, for searching.
    keys: [String; 4],
}

pub struct City {
    pub name: String,
    pub position: Vec3,
    /// In thousands.
    pub population: f32,
}

fn degrees(value: &str) -> f32 {
    value.parse::<f32>().unwrap_or(0.0) / 100.0
}

fn position(lon: f32, lat: f32) -> Vec3 {
    surface(lon.to_radians(), lat.to_radians())
}

pub fn airports() -> Vec<Airport> {
    include_str!("data/airports.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.splitn(4, ' ');
            let code = fields.next()?.to_string();
            let lon = degrees(fields.next()?);
            let lat = degrees(fields.next()?);
            let mut names = fields.next()?.split('|').map(str::to_string);
            let (city, country, name) = (names.next()?, names.next()?, names.next()?);
            let keys = [&code, &city, &country, &name].map(|text| fold(text));
            Some(Airport { code, city, country, name, position: position(lon, lat), keys })
        })
        .collect()
}

/// The cities with at least `min_population` thousand people, the largest first.
pub fn cities(min_population: f32) -> Vec<City> {
    let mut cities: Vec<City> = include_str!("data/cities.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.splitn(4, ' ');
            let lon = degrees(fields.next()?);
            let lat = degrees(fields.next()?);
            let population = fields.next()?.parse().ok()?;
            let name = fields.next()?.to_string();
            Some(City { name, position: position(lon, lat), population })
        })
        .filter(|city| city.population >= min_population)
        .collect();
    cities.sort_by(|a, b| b.population.total_cmp(&a.population));
    cities
}

/// `text` in lowercase, without the accents of the Latin letters, so that "sao" finds "São".
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è'..='ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'ì'..='ï' | 'ī' | 'į' | 'ı' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ś' | 'š' | 'ş' => 's',
            'ź' | 'ż' | 'ž' => 'z',
            'ł' => 'l',
            'ř' => 'r',
            'ğ' => 'g',
            other => other,
        })
        .collect()
}

/// The airports that match `query`, the best matches first: the code, then cities, names,
/// and countries that start with it, then any that contain it.
pub fn search(airports: &[Airport], query: &str) -> Vec<usize> {
    let query = fold(query.trim());
    let rank = |airport: &Airport| -> Option<u8> {
        if query.is_empty() {
            return Some(0);
        }
        let [code, city, country, name] = &airport.keys;
        let starts_word = |text: &str| {
            text.split(|c: char| !c.is_alphanumeric()).any(|word| word.starts_with(&query))
        };
        if *code == query {
            Some(0)
        } else if city.starts_with(&query) {
            Some(1)
        } else if starts_word(name) || starts_word(city) {
            Some(2)
        } else if country.starts_with(&query) {
            Some(3)
        } else if airport.keys.iter().any(|key| key.contains(&query)) {
            Some(4)
        } else {
            None
        }
    };
    let mut matches: Vec<(u8, usize)> = airports
        .iter()
        .enumerate()
        .filter_map(|(index, airport)| Some((rank(airport)?, index)))
        .collect();
    matches.sort_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| airports[a.1].keys[1].cmp(&airports[b.1].keys[1]))
    });
    matches.into_iter().map(|(_, index)| index).collect()
}
