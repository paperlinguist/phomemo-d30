//! The emoji to choose from: in groups, with words to find them by.
//!
//! Each emoji is a single character, as printed labels cannot show the ones that are made of several
//! (families, flags). The window only offers those that the fonts can draw: see
//! [`d30::fonts::Fonts::fallback_draws`].

/// An emoji, and what it is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emoji {
    pub symbol: char,
    /// Its name and some words to find it by, in lower case.
    pub name: &'static str,
}

/// A group of emoji.
pub struct Group {
    pub name: &'static str,
    /// What the button for the group shows.
    pub icon: char,
    /// One emoji to a line: the emoji, a space, then its name and words to find it by.
    entries: &'static str,
}

impl Group {
    pub fn emoji(&self) -> impl Iterator<Item = Emoji> {
        self.entries.lines().filter_map(parse)
    }
}

/// An emoji from a line of a group, if the line is one.
fn parse(line: &'static str) -> Option<Emoji> {
    let (symbol, name) = line.trim().split_once(' ')?;
    let mut characters = symbol.chars();
    let first = characters.next()?;
    // More than one character is an emoji that cannot be drawn on a label.
    characters.next().is_none().then_some(Emoji {
        symbol: first,
        name: name.trim(),
    })
}

/// All the emoji (of any group) whose names have each of the words of `query` in them. The
/// capitals of `query` do not matter.
pub fn search(query: &str) -> Vec<Emoji> {
    let query = query.to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    GROUPS
        .iter()
        .flat_map(Group::emoji)
        .filter(|emoji| words.iter().all(|word| emoji.name.contains(word)))
        .collect()
}

pub const GROUPS: &[Group] = &[
    Group {
        name: "Food and drink",
        icon: '🍎',
        entries: FOOD,
    },
    Group {
        name: "Home and things",
        icon: '🏠',
        entries: HOME,
    },
    Group {
        name: "Office and tools",
        icon: '📎',
        entries: OFFICE,
    },
    Group {
        name: "Nature and animals",
        icon: '🌱',
        entries: NATURE,
    },
    Group {
        name: "Faces and hearts",
        icon: '😀',
        entries: FACES,
    },
    Group {
        name: "Signs and symbols",
        icon: '⚠',
        entries: SIGNS,
    },
];

const FOOD: &str = "
🍎 apple red fruit
🍏 green apple fruit
🍐 pear fruit
🍊 orange tangerine citrus fruit
🍋 lemon citrus fruit
🍌 banana fruit
🍉 watermelon melon fruit
🍇 grapes fruit
🍓 strawberry berry fruit
🫐 blueberries berries fruit
🍈 melon fruit
🍒 cherries cherry fruit
🍑 peach fruit
🥭 mango fruit
🍍 pineapple fruit
🥥 coconut
🥝 kiwi fruit
🍅 tomato vegetable
🍆 eggplant aubergine vegetable
🥑 avocado
🥦 broccoli vegetable
🥬 leafy green lettuce cabbage salad
🥒 cucumber pickle vegetable
🌶 hot pepper chili spicy
🫑 bell pepper capsicum vegetable
🌽 corn maize
🥕 carrot vegetable
🧄 garlic
🧅 onion
🥔 potato
🍠 sweet potato
🫘 beans kidney legumes
🥜 peanuts nuts
🌰 chestnut nut
🫒 olive
🍄 mushroom
🍞 bread loaf
🥐 croissant pastry
🥖 baguette french bread
🥨 pretzel
🥯 bagel
🥞 pancakes
🧇 waffle
🧀 cheese
🍖 meat bone
🍗 chicken poultry drumstick
🥩 steak cut of meat
🥓 bacon
🍔 hamburger burger
🍟 french fries chips
🍕 pizza
🌭 hot dog sausage
🥪 sandwich
🌮 taco
🌯 burrito wrap
🥗 salad green
🥘 paella pan stew
🍝 spaghetti pasta
🍜 noodles ramen soup
🍲 pot of food stew
🍛 curry rice
🍣 sushi
🍱 bento box
🥟 dumpling
🍤 shrimp prawn
🍙 rice ball onigiri
🍚 rice bowl
🍘 rice cracker
🍥 fish cake
🥠 fortune cookie
🍢 oden skewer
🍡 dango sweet
🥚 egg
🍳 cooking fried egg pan
🧈 butter
🥣 bowl with spoon cereal porridge
🍧 shaved ice
🍨 ice cream
🍦 soft ice cream
🥧 pie
🧁 cupcake
🍰 cake slice
🎂 birthday cake
🍮 custard pudding flan
🍭 lollipop candy
🍬 candy sweet
🍫 chocolate bar
🍿 popcorn
🍩 doughnut donut
🍪 cookie biscuit
🍯 honey pot
🧂 salt shaker seasoning
🫙 jar jam storage
🥫 canned food tin
🍼 baby bottle milk
🥛 glass of milk
☕ coffee tea hot drink
🫖 teapot tea
🍵 green tea cup matcha
🍶 sake
🍾 champagne bottle
🍷 wine glass red
🍸 cocktail glass martini
🍹 tropical drink
🍺 beer mug
🍻 beers cheers
🥂 clinking glasses toast
🥃 whisky tumbler glass
🥤 cup with straw soda
🧋 bubble tea boba
🧃 juice box
🧉 mate drink
🧊 ice cube
🥢 chopsticks
🍽 plate fork knife dinner
🍴 fork and knife cutlery
🥄 spoon
";

const HOME: &str = "
🏠 house home
🏡 house with garden
🏢 office building
🏪 shop store
🏫 school
🏥 hospital
🏭 factory
⛪ church
🏰 castle
🔑 key
🗝 old key
🚪 door
🪟 window
🛏 bed
🛋 couch sofa lamp
🪑 chair
🚽 toilet
🚿 shower
🛁 bathtub bath
🪞 mirror
🧴 lotion bottle soap
🧼 soap
🧽 sponge
🧹 broom sweep
🧺 laundry basket
🧻 toilet paper roll
🪣 bucket
🪥 toothbrush
🪒 razor
🧯 fire extinguisher
🛒 shopping cart trolley
💡 light bulb idea
🔦 flashlight torch
🕯 candle
🔌 plug electric
🔋 battery
🪫 low battery
⏰ alarm clock
🕰 mantel clock
🧸 teddy bear toy
🪴 potted plant
🖼 framed picture
📺 television tv
📻 radio
📞 telephone phone
📱 mobile phone
☎ telephone
💻 laptop computer
🖥 desktop computer
🖨 printer
⌨ keyboard
🖱 computer mouse
💾 floppy disk save
💿 cd disc
📷 camera
🎥 movie camera
🎧 headphones
🔊 speaker loud
🎮 video game controller
🧳 luggage suitcase
🎒 backpack bag school
👜 handbag
🛍 shopping bags
🧰 toolbox
📦 package box parcel
🗑 wastebasket trash bin
🧲 magnet
🪜 ladder
🧱 brick
🪵 wood log
🛠 hammer and wrench repair
";

const OFFICE: &str = "
📎 paperclip
🖇 linked paperclips
📌 pushpin
📍 round pushpin
✂ scissors cut
📏 ruler
📐 triangular ruler
✏ pencil
🖊 pen
🖋 fountain pen
🖌 paintbrush
🖍 crayon
📝 memo note
📓 notebook
📔 notebook decorative
📕 closed book red
📗 green book
📘 blue book
📙 orange book
📚 books
📖 open book
🔖 bookmark
🏷 label tag price
📂 open folder
📁 folder
🗂 card index dividers
🗃 card file box
🗄 file cabinet
📅 calendar date
📆 tear-off calendar
🗓 spiral calendar
📇 card index
📈 chart increasing
📉 chart decreasing
📊 bar chart
📋 clipboard
📃 page with curl
📄 page document
📰 newspaper
🗞 rolled newspaper
✉ envelope mail letter
📧 email
📨 incoming envelope
📩 envelope with arrow
📬 mailbox
📮 postbox
🔨 hammer
🔧 wrench spanner
🪛 screwdriver
🔩 nut and bolt screw
⚙ gear cog settings
🪚 saw
⛏ pick
🪓 axe
🧵 thread spool sewing
🧶 yarn wool knitting
🪡 sewing needle
🧷 safety pin
🔗 link chain
⛓ chains
📡 satellite antenna
🔬 microscope
🔭 telescope
💊 pill medicine
💉 syringe injection
🩹 bandage plaster
🩺 stethoscope
🌡 thermometer
🧪 test tube
🧫 petri dish
🧬 dna
💰 money bag
💵 dollar banknote cash
💳 credit card
🧾 receipt
🔒 locked padlock
🔓 unlocked
🔐 lock with key
🔔 bell
🔕 bell with slash muted
📢 loudspeaker
📣 megaphone
⏳ hourglass
⌛ hourglass done
⏱ stopwatch
⏲ timer
🧭 compass
";

const NATURE: &str = "
🌱 seedling sprout plant
🌿 herb leaf
☘ shamrock
🍀 four leaf clover luck
🌵 cactus
🌲 evergreen tree
🌳 deciduous tree
🌴 palm tree
🌸 cherry blossom
💐 bouquet flowers
🌹 rose
🌺 hibiscus
🌻 sunflower
🌼 daisy blossom
🌷 tulip
🍁 maple leaf autumn
🍂 fallen leaf
🍃 leaf in wind
☀ sun sunny
🌙 crescent moon night
⭐ star
🌟 glowing star
☁ cloud
⛅ sun behind cloud
⛈ thunderstorm
🌧 rain
❄ snowflake snow cold
☃ snowman
⛄ snowman without snow
🔥 fire flame hot
💧 droplet water
🌊 wave sea
🌈 rainbow
⚡ lightning bolt electric
🌍 earth globe
🐶 dog
🐱 cat
🐭 mouse
🐹 hamster
🐰 rabbit bunny
🦊 fox
🐻 bear
🐼 panda
🐨 koala
🐯 tiger
🦁 lion
🐮 cow
🐷 pig
🐸 frog
🐵 monkey
🐔 chicken
🐧 penguin
🐦 bird
🦆 duck
🦉 owl
🐝 bee honeybee
🦋 butterfly
🐞 ladybug
🐛 bug caterpillar
🐌 snail
🐢 turtle
🐍 snake
🐟 fish
🐠 tropical fish
🐳 whale
🐬 dolphin
🐘 elephant
🐴 horse
🦄 unicorn
🐾 paw prints pet
🦴 bone dog
";

const FACES: &str = "
😀 grinning face smile
😃 smiling face open mouth
😄 smiling eyes happy
😁 beaming face
😆 laughing
😅 sweat smile
😂 tears of joy laugh
🙂 slight smile
😉 wink
😊 blush smile
😇 angel halo
😍 heart eyes love
😘 blowing kiss
😋 yum tasty delicious
😎 cool sunglasses
🤓 nerd glasses
🤔 thinking
😐 neutral face
😴 sleeping
😷 mask sick
🤒 fever ill
😱 scream fear
😡 angry
😢 crying sad
😭 sobbing
🙄 eye roll
😬 grimace
🥳 party celebrate
🤩 star struck
🥰 smiling with hearts
😈 devil
💀 skull
☠ skull and crossbones poison
👻 ghost
🤖 robot
💩 poop
❤ red heart love
🧡 orange heart
💛 yellow heart
💚 green heart
💙 blue heart
💜 purple heart
🖤 black heart
🤍 white heart
💔 broken heart
💯 hundred points
✨ sparkles clean
💥 collision boom
💫 dizzy
💤 zzz sleep
👍 thumbs up like good
👎 thumbs down dislike
👏 clapping hands
🙏 folded hands please thanks
👋 waving hand hello
✌ victory peace
👌 ok hand
☝ pointing up
👆 backhand index up
👇 backhand index down
👈 backhand index left
👉 backhand index right
💪 flexed biceps strong
👀 eyes
👶 baby
👨 man
👩 woman
🧒 child
👴 old man
👵 old woman
";

const SIGNS: &str = "
✅ check mark button done
✔ check mark tick
❌ cross mark
✖ multiply
⭕ circle
⚠ warning caution
⛔ no entry
🚫 prohibited forbidden
❓ question
❔ white question
❗ exclamation
❕ white exclamation
➕ plus
➖ minus
➗ divide
➡ right arrow
⬅ left arrow
⬆ up arrow
⬇ down arrow
↔ left right arrow
↕ up down arrow
🔄 counterclockwise arrows
🔁 repeat
♻ recycling recycle
⚛ atom
☢ radioactive
☣ biohazard
☮ peace
☯ yin yang
♿ wheelchair accessible
🚻 restroom toilets
🚹 men
🚺 women
🚼 baby symbol
🔞 no under eighteen
📵 no mobile phones
🚭 no smoking
🚯 no littering
🚱 non-potable water
🚳 no bicycles
🔇 muted speaker
ℹ information
🆘 sos
🆗 ok button
🆕 new
🆓 free
🔝 top
🔜 soon
🔙 back
🔛 on
🔚 end
🔴 red circle
🟠 orange circle
🟡 yellow circle
🟢 green circle
🔵 blue circle
🟣 purple circle
⚫ black circle
⚪ white circle
🟥 red square
🟧 orange square
🟨 yellow square
🟩 green square
🟦 blue square
🟪 purple square
⬛ black square
⬜ white square
🔺 red triangle up
🔻 red triangle down
🔶 large orange diamond
🔷 large blue diamond
♠ spade
♣ club
♥ heart suit
♦ diamond suit
🎁 gift present
🎈 balloon
🎉 party popper
🎊 confetti
🏆 trophy
🥇 gold medal first
🎵 music note
🎶 musical notes
⚽ soccer football
🏀 basketball
🎯 bullseye target
🎲 dice game
🧩 puzzle piece
🎨 palette art
🏳 white flag
🚩 triangular flag
🏁 chequered flag
✈ airplane
🚗 car
🚚 delivery truck
🚲 bicycle
⚓ anchor
⛽ fuel pump
🗺 world map
";

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use d30::fonts::Fonts;

    use super::*;

    fn all() -> Vec<Emoji> {
        GROUPS.iter().flat_map(Group::emoji).collect()
    }

    #[test]
    fn every_line_of_every_group_is_an_emoji_with_a_name() {
        for group in GROUPS {
            let lines = group
                .entries
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            assert_eq!(
                group.emoji().count(),
                lines,
                "a line of {} is not an emoji",
                group.name
            );
            assert!(lines >= 40, "{} has only {lines}", group.name);
            assert!(
                group.emoji().any(|emoji| emoji.symbol == group.icon),
                "the icon of {} is not one of its emoji",
                group.name
            );
        }
        for emoji in all() {
            assert!(
                !emoji.name.is_empty() && emoji.name == emoji.name.to_lowercase(),
                "{emoji:?}"
            );
        }
    }

    #[test]
    fn no_emoji_is_in_the_list_twice() {
        let mut seen = HashSet::new();
        for emoji in all() {
            assert!(
                seen.insert(emoji.symbol),
                "{} is listed twice",
                emoji.symbol
            );
        }
        assert!(seen.len() > 400, "{} emoji", seen.len());
    }

    #[test]
    fn an_emoji_is_one_character_and_never_a_selector_or_a_joiner() {
        for emoji in all() {
            let code = emoji.symbol as u32;
            assert!(
                !matches!(code, 0xfe0e | 0xfe0f | 0x200d | 0x1f3fb..=0x1f3ff),
                "{:04X}",
                code
            );
        }
        // A line with two characters in it is not an emoji, as printing it would not work.
        assert_eq!(parse("🏳️‍🌈 rainbow flag"), None);
        assert_eq!(parse("🍎 apple").map(|emoji| emoji.symbol), Some('🍎'));
        assert_eq!(parse("🍎"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn emoji_are_found_by_any_words_of_their_names_whatever_the_capitals() {
        let found = |query: &str| -> Vec<char> {
            search(query)
                .into_iter()
                .map(|emoji| emoji.symbol)
                .collect()
        };
        assert!(found("apple").contains(&'🍎'));
        assert!(found("APPLE").contains(&'🍎'));
        assert!(found("red apple").contains(&'🍎'));
        assert!(!found("red apple").contains(&'🍏'));
        assert!(found("carrot").contains(&'🥕'));
        assert!(found("jam").contains(&'🫙'));
        // Part of a word will do, which is how a search finds as you type.
        assert!(found("carr").contains(&'🥕'));
        assert!(found("   ").is_empty());
        assert!(found("").is_empty());
        assert!(found("zzzzzz").is_empty());
    }

    #[test]
    fn what_the_built_in_font_can_draw_is_most_of_the_list_and_can_be_told_apart() {
        // Without the installed fonts only the built-in emoji font draws them, and it is old.
        let fonts = Fonts::embedded();
        let drawn = all()
            .into_iter()
            .filter(|emoji| fonts.fallback_draws(emoji.symbol))
            .count();
        assert!(drawn > 200, "only {drawn} of the emoji can be drawn");
        assert!(fonts.fallback_draws('🍎'));
        assert!(!fonts.fallback_draws('x'));
    }

    #[test]
    fn with_the_fonts_of_this_computer_nearly_all_of_the_list_can_be_drawn() {
        let fonts = Fonts::system();
        // Only where a recent emoji font is installed, as Windows has.
        if fonts.installed_emoji_font().is_none() {
            return;
        }
        let missing: Vec<char> = all()
            .into_iter()
            .map(|emoji| emoji.symbol)
            .filter(|&symbol| !fonts.fallback_draws(symbol))
            .collect();
        assert!(
            missing.len() < 12,
            "{} emoji cannot be drawn: {missing:?}",
            missing.len()
        );
    }
}
