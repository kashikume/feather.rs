use anyhow::{anyhow, Result};

use feather::animation::SlideAnimation;
use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec2, Vec3};
use feather::mesh::Mesh;
use feather::scene::Scene;
use feather::text::{
    create_text_material, HorizontalAlign, Text, TextBuilder, TextPlane, VerticalAlign,
};
use feather::vertex::Vertex;

use super::random::Random;
use super::testapp::Demo;

/// How fast the text moves away, in units per second.
const SPEED: f32 = 0.55;
/// World Z where a new episode's top starts: just below the bottom of the screen.
const START_Z: f32 = 7.5;
/// Space between the end of one episode and the start of the next.
const EPISODE_GAP: f32 = 2.2;
/// An episode is removed once its bottom has moved beyond this Z, where the text shader has
/// faded it out (the `Crawl` node's `fadeEnd`, with a margin).
const REMOVE_Z: f32 = -26.0;

/// Width of the paragraphs, which are justified.
const WIDTH: f32 = 5.2;
const EPISODE_HEIGHT: f32 = 0.4;
const TITLE_HEIGHT: f32 = 0.7;
const PARAGRAPH_HEIGHT: f32 = 0.34;
/// Space between the parts of an episode.
const PART_GAP: f32 = 0.45;

/// Stars, on a wall far behind the text.
const STARS: usize = 1600;
const STAR_WALL_Z: f32 = -70.0;

/// Demo 11: an endless crawl. Yellow text lying on an invisible floor slides away from the
/// viewer towards the horizon, fading out, over a field of twinkling stars. Episodes are made
/// up as they are needed, with numbers going up forever: an episode number, a title and three
/// justified paragraphs.
///
/// The scene file has the font atlas (Segoe UI Bold, rendered during the build from
/// `atlases/demo_011_crawl_atlas.font`), the shaders, the stars' material and the `Stars` and
/// `Crawl` nodes. This demo builds the star field (one mesh of tiny squares) and adds a node per
/// episode under `Crawl`, with a `Text` per part (one mesh each, in the XZ plane). The text
/// shader fades the text with its distance, between the `fadeStart` and `fadeEnd` uniforms of
/// `Crawl`.
///
/// Each episode moves by a scene animation (`SlideAnimation`, linear) from where it starts to
/// where it has faded out, and its `on_finish` removes it, texts included (they are moved into
/// the callback). The demo only adds episodes: it remembers the newest one and adds the next once
/// that one has moved far enough. Both move in the same animation update, so the gap between
/// them stays as set.
pub struct CrawlDemo {
    scene: Scene,
    random: Random,
    font: usize,
    material: usize,
    crawl: usize,
    stars: usize,
    /// The newest episode: its node and its length, from its top to its bottom along +Z.
    newest: Option<(usize, f32)>,
    /// The number of the next episode.
    next_number: u32,
}

impl CrawlDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        Self::with_random(assets, Random::from_clock())
    }

    /// The demo with stories made from `random`.
    pub fn with_random(assets: &dyn AssetsProvider, mut random: Random) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_011_crawl_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let crawl = find("node `Crawl`", scene.find_node("Crawl"))?;
        let stars = find("node `Stars`", scene.find_node("Stars"))?;
        let font = find("atlas `Font`", scene.find_atlas("Font"))?;
        let shader = find("shader `Text`", scene.find_shader("Text"))?;
        let material = create_text_material(&mut scene, Some("Text".into()), shader, font)?;

        let star_mesh = scene.add_mesh(star_field(&mut random));
        scene.node_set_mesh(stars, star_mesh)?;

        // The scene file's camera: low above the floor the text lies on, looking ahead and a little
        // down.
        let mut demo = Self {
            scene,
            random,
            font,
            material,
            crawl,
            stars,
            newest: None,
            next_number: 1,
        };
        demo.update(0.0)?;
        Ok(demo)
    }

    /// Adds the next episode with its top at `z`, and the animation moving it away until it has
    /// faded out, then removing it.
    fn add_episode(&mut self, z: f32) -> Result<()> {
        let number = self.next_number;
        self.next_number += 1;
        let story = Story::make(&mut self.random, number);

        let node = self
            .scene
            .create_node(Some(format!("Episode {number}")), self.crawl)?;
        let centred = |height| {
            TextBuilder::new(self.font)
                .height(height)
                .plane(TextPlane::XZ)
                .align(HorizontalAlign::Center, VerticalAlign::Top)
        };
        // Each part with the x of its origin: centred parts have it in their middle, the
        // justified paragraphs at their left edge.
        let parts = [
            (centred(EPISODE_HEIGHT), story.episode, 0.0),
            (centred(TITLE_HEIGHT), story.title, 0.0),
            (
                TextBuilder::new(self.font)
                    .height(PARAGRAPH_HEIGHT)
                    .line_spacing(1.2)
                    .plane(TextPlane::XZ)
                    .wrap(Some(WIDTH))
                    .align(HorizontalAlign::Justify, VerticalAlign::Top),
                story.paragraphs.join("\n\n"),
                -WIDTH / 2.0,
            ),
        ];
        let mut texts = Vec::new();
        let mut length = 0.0;
        for (builder, text, x) in parts {
            if !texts.is_empty() {
                length += PART_GAP;
            }
            let text = Text::create(&mut self.scene, node, None, self.material, builder, &text)?;
            self.scene
                .get_node_mut(text.get_node())
                .unwrap()
                .set_transform(Mat4::from_translation(Vec3::new(x, 0.0, length)));
            length += text.get_size().height;
            texts.push(text);
        }

        // Its top goes from `z` to where its bottom reaches `REMOVE_Z`, at `SPEED`.
        let end = REMOVE_Z - length;
        let place = move |scene: &mut Scene, top: f32| {
            if let Some(n) = scene.get_node_mut(node) {
                n.set_transform(Mat4::from_translation(Vec3::new(0.0, 0.0, top)));
            }
        };
        place(&mut self.scene, z);
        self.scene.add_animation(
            None,
            SlideAnimation::new((z - end) / SPEED, move |scene: &mut Scene, p| {
                place(scene, z + (end - z) * p)
            })
            .on_finish(move |scene: &mut Scene| {
                // The texts' meshes go with them; removing the node also cancels this
                // animation's entry.
                for text in texts {
                    if let Err(e) = text.remove(scene) {
                        log::error!("Cannot remove episode {number}'s text: {e:#}");
                    }
                }
                let _ = scene.remove_node(node);
            })
            .target(node),
        );
        self.newest = Some((node, length));
        Ok(())
    }

    /// Where the newest episode's bottom is now, if it is still there.
    fn newest_bottom(&self) -> Option<f32> {
        let (node, length) = self.newest?;
        let top = self.scene.get_node(node)?.get_transform().w.z;
        Some(top + length)
    }
}

impl Demo for CrawlDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // A new episode follows when the newest one has moved far enough. The first starts a
        // little way up the screen, so there is something to read.
        match self.newest_bottom() {
            None => self.add_episode(START_Z - 4.0)?,
            Some(bottom) if bottom + EPISODE_GAP <= START_Z => {
                self.add_episode(bottom + EPISODE_GAP)?
            }
            Some(_) => {}
        }
        self.scene
            .get_node_mut(self.stars)
            .unwrap()
            .uniforms_mut()
            .set("time", time);
        Ok(())
    }
}

/// One mesh of tiny squares facing +Z on a wall far behind the text, each with its brightness
/// and twinkle phase in its texture coordinates.
fn star_field(random: &mut Random) -> Mesh {
    let mut vertices = Vec::with_capacity(STARS * 4);
    let mut indices = Vec::with_capacity(STARS * 6);
    let normal = Vec3::new(0.0, 0.0, 1.0);
    for _ in 0..STARS {
        let (x, y) = (random.range(-80.0, 80.0), random.range(-60.0, 60.0));
        // Most stars faint and small, a few bright and bigger.
        let brightness = random.next().powi(3) * 0.85 + 0.15;
        let size = 0.07 + 0.12 * brightness;
        let star = Vec2::new(brightness, random.next());
        let first = vertices.len() as u32;
        for (dx, dy) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let position = Vec3::new(x + dx * size, y + dy * size, STAR_WALL_Z);
            vertices.push(Vertex::new(position, normal, star));
        }
        // Top left, top right, bottom left, bottom right; counter-clockwise from +Z.
        indices.extend([first + 2, first + 3, first + 1, first + 2, first + 1, first]);
    }
    Mesh::new(Some("Stars".into()), vertices, indices)
}

/// The text of an episode, made up from word lists.
struct Story {
    episode: String,
    title: String,
    paragraphs: Vec<String>,
}

impl Story {
    fn make(random: &mut Random, number: u32) -> Story {
        let mut pick = |list: &[&'static str]| list[(random.next() * list.len() as f32) as usize];

        let title = format!("THE {} {}", pick(ADJECTIVES), pick(NOUNS));
        let faction = pick(FACTIONS);
        let mut rival = pick(FACTIONS);
        while rival == faction {
            rival = pick(FACTIONS);
        }
        let hero = pick(HEROES);
        let first = format!(
            "It is a time of {}. {} has {} across {}, and the old routes between the stars are falling silent.",
            pick(TIMES),
            capitalize(faction),
            pick(DEEDS),
            pick(PLACES),
        );
        let second = format!(
            "{} has escaped with {}, the only hope of {}.",
            capitalize(hero),
            pick(OBJECTS),
            pick(HOPES),
        );
        let third = format!(
            "Pursued by the ships of {}, the fugitive races toward {}, where {}…",
            rival,
            pick(PLACES),
            pick(ENDINGS),
        );
        Story {
            episode: format!("EPISODE {}", roman(number)),
            title,
            paragraphs: vec![first, second, third],
        }
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `number` in Roman numerals (with as many `M`s as needed past 3999).
fn roman(mut number: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut text = String::new();
    for (value, numeral) in NUMERALS {
        while number >= value {
            text.push_str(numeral);
            number -= value;
        }
    }
    text
}

const ADJECTIVES: &[&str] = &[
    "SILENT",
    "FORGOTTEN",
    "CRIMSON",
    "ENDLESS",
    "HIDDEN",
    "LAST",
    "DISTANT",
    "FROZEN",
    "BURNING",
    "WANDERING",
    "HOLLOW",
    "GOLDEN",
    "BROKEN",
    "SLEEPING",
];
const NOUNS: &[&str] = &[
    "SIGNAL", "COMET", "ARCHIVE", "HARBOR", "ORBIT", "LANTERN", "FRONTIER", "ENGINE", "NEBULA",
    "COVENANT", "BEACON", "TIDE", "CROWN", "VOYAGE",
];
const TIMES: &[&str] = &[
    "uneasy peace",
    "slow decline",
    "quiet rebellion",
    "great migration",
    "broken treaties",
    "long winters",
    "failing light",
];
const FACTIONS: &[&str] = &[
    "the Merchant Guild",
    "the Cartographers' League",
    "the Ember Syndicate",
    "the Lantern Order",
    "the Assembly of Worlds",
    "the Iron Concord",
    "the Pale Fleet",
];
const DEEDS: &[&str] = &[
    "seized the trade lanes",
    "closed every jump gate",
    "outlawed the old star charts",
    "doubled the tolls on every port",
    "silenced the relay stations",
    "taken the shipyards",
];
const PLACES: &[&str] = &[
    "the Veyra system",
    "the Halcyon Drift",
    "the free city of Ostmark",
    "the moons of Tavren",
    "the Glass Expanse",
    "the Sunken Belt",
    "the far colonies of Aurel",
];
const HEROES: &[&str] = &[
    "captain Iri Vossen",
    "the engineer Malo Dask",
    "the pilot Sera Quill",
    "old admiral Brann",
    "the archivist Teo Lume",
    "the smuggler Kett Arro",
];
const OBJECTS: &[&str] = &[
    "a stolen star chart",
    "the last working jump drive",
    "an ancient signal beacon",
    "a crate of seeds from a lost world",
    "the map of the hidden lanes",
    "the key to the sealed archive",
];
const HOPES: &[&str] = &[
    "reopening the lanes",
    "finding the lost colonies",
    "ending the long blockade",
    "bringing light back to the outer worlds",
    "proving the old stories true",
];
const ENDINGS: &[&str] = &[
    "an old friend may still be waiting",
    "the answer has been hidden for a hundred years",
    "the fate of a thousand worlds will be decided",
    "a forgotten fleet lies sleeping",
    "the last free port still keeps its lights on",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roman_numerals() {
        let cases = [
            (1, "I"),
            (4, "IV"),
            (9, "IX"),
            (14, "XIV"),
            (40, "XL"),
            (1994, "MCMXCIV"),
        ];
        for (number, text) in cases {
            assert_eq!(roman(number), text);
        }
        assert_eq!(roman(5000), "MMMMM");
    }

    #[test]
    fn stories_are_made_up_from_the_lists() {
        let mut random = Random::from_seed(3);
        let story = Story::make(&mut random, 12);
        assert_eq!(story.episode, "EPISODE XII");
        assert!(story.title.starts_with("THE "));
        assert_eq!(story.paragraphs.len(), 3);
        assert!(story.paragraphs[0].starts_with("It is a time of "));
        let other = Story::make(&mut random, 13);
        assert_ne!(
            (story.title.clone(), story.paragraphs.clone()),
            (other.title, other.paragraphs)
        );
    }

    #[test]
    fn capitalize_first_letter() {
        assert_eq!(capitalize("the pilot"), "The pilot");
        assert_eq!(capitalize(""), "");
    }
}
