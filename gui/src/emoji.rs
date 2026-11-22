//! The emoji to choose from: in groups, with words to find them by.
//!
//! The names, the groups and the words to find them by come from the Unicode Consortium: its list of
//! emoji (`emoji-test.txt`, Unicode 18) and the English annotations of the Common Locale Data
//! Repository (CLDR), which are under the Unicode License (<https://www.unicode.org/license.txt>), and
//! to which the words of the list that was here before were added. Each emoji is a single character,
//! as printed labels cannot show the ones that are made of several (families, flags, skin tones). The
//! window only offers those that the fonts can draw: see [`d30::fonts::Fonts::fallback_draws`].

/// An emoji, and what it is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emoji {
    pub symbol: char,
    /// What it is called, in lower case.
    pub name: &'static str,
    /// More words to find it by, in lower case. Can be empty.
    pub words: &'static str,
}

/// A group of emoji.
pub struct Group {
    pub name: &'static str,
    /// What the button for the group shows.
    pub icon: char,
    /// One emoji to a line: the emoji, a space, its name, and after a `|` words to find it by.
    entries: &'static str,
}

impl Group {
    pub fn emoji(&self) -> impl Iterator<Item = Emoji> {
        self.entries.lines().filter_map(parse)
    }
}

/// An emoji from a line of a group, if the line is one.
fn parse(line: &'static str) -> Option<Emoji> {
    let (symbol, rest) = line.trim().split_once(' ')?;
    let mut characters = symbol.chars();
    let first = characters.next()?;
    // More than one character is an emoji that cannot be drawn on a label.
    if characters.next().is_some() {
        return None;
    }
    let (name, words) = rest.split_once('|').unwrap_or((rest, ""));
    Some(Emoji {
        symbol: first,
        name: name.trim(),
        words: words.trim(),
    })
}

/// All the emoji (of any group) for which each of the words of `query` is the start of a word of
/// their names or of the words to find them by: "carr" finds the carrot, and "apple" the apples but
/// not the people who grapple. The capitals of `query` do not matter.
pub fn search(query: &str) -> Vec<Emoji> {
    let query = query.to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    GROUPS
        .iter()
        .flat_map(Group::emoji)
        .filter(|emoji| {
            words.iter().all(|word| {
                emoji
                    .name
                    .split_whitespace()
                    .chain(emoji.words.split_whitespace())
                    .any(|candidate| candidate.starts_with(word))
            })
        })
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
        name: "People and hands",
        icon: '👋',
        entries: PEOPLE,
    },
    Group {
        name: "Travel and places",
        icon: '🚗',
        entries: TRAVEL,
    },
    Group {
        name: "Activities and music",
        icon: '⚽',
        entries: ACTIVITIES,
    },
    Group {
        name: "Signs and symbols",
        icon: '⚠',
        entries: SIGNS,
    },
];

const FOOD: &str = "
🍇 grapes | dionysus fruit grape
🍈 melon | cantaloupe fruit
🍉 watermelon | fruit melon
🍊 tangerine | c citrus fruit nectarine orange vitamin
🍋 lemon | citrus fruit sour
🍌 banana | fruit potassium
🍍 pineapple | colada fruit pina tropical
🥭 mango | food fruit tropical
🍎 red apple | diet food fruit health ripe
🍏 green apple | fruit
🍐 pear | fruit
🍑 peach | fruit
🍒 cherries | berries cherry fruit red
🍓 strawberry | berry fruit
🫐 blueberries | berries berry bilberry blue blueberry food fruit
🥝 kiwi fruit | food
🍅 tomato | food fruit vegetable
🫒 olive | food
🥥 coconut | colada palm piña
🥑 avocado | food fruit
🍆 eggplant | aubergine vegetable
🥔 potato | food vegetable
🥕 carrot | food vegetable
🌽 ear of corn | crops farm maize maze
🌶 hot pepper | chili spicy
🫑 bell pepper | capsicum food vegetable
🥒 cucumber | food pickle vegetable
🫝 pickle | brine condiment cucumber ferment gherkin relish trouble
🥬 leafy green | bok burgers cabbage choy kale lettuce salad
🥦 broccoli | cabbage wild vegetable
🧄 garlic | flavoring
🧅 onion | flavoring
🥜 peanuts | food nut peanut vegetable nuts
🫘 beans | food kidney legume small legumes
🌰 chestnut | almond plant nut
🫚 ginger root | beer health herb natural spice
🫛 pea pod | beans beanstalk edamame legume soybean vegetable veggie
🫜 root vegetable | beet food garden radish salad turnip vegetarian
🍞 bread | carbs food grain loaf restaurant toast wheat
🥐 croissant | bread breakfast crescent food french roll pastry
🥖 baguette bread | food french
🫓 flatbread | arepa bread food gordita lavash naan pita
🥨 pretzel | convoluted twisted
🥯 bagel | bakery bread breakfast schmear
🥞 pancakes | breakfast crêpe food hotcake pancake
🧇 waffle | breakfast indecisive iron
🧀 cheese wedge
🍖 meat on bone
🍗 poultry leg | bone chicken drumstick hungry turkey
🥩 cut of meat | chop lambchop porkchop red steak
🥓 bacon | breakfast food meat
🍔 hamburger | burger eat fast food hungry
🍟 french fries | fast food chips
🍕 pizza | cheese food hungry pepperoni slice
🌭 hot dog | frankfurter hotdog sausage
🥪 sandwich | bread
🌮 taco | mexican
🌯 burrito | mexican wrap
🫔 tamale | food mexican pamonha wrapped
🥙 stuffed flatbread | falafel food gyro kebab
🧆 falafel | chickpea meatball
🥚 egg | breakfast food
🍳 cooking | breakfast easy egg fry frying over pan restaurant side sunny up fried
🥘 shallow pan of food | casserole paella stew
🍲 pot of food | soup stew
🫕 fondue | cheese chocolate food melted pot ski
🥣 bowl with spoon | breakfast cereal congee oatmeal porridge
🥗 green salad | food
🍿 popcorn | corn movie pop
🧈 butter | dairy
🧂 salt | condiment flavor mad salty shaker taste upset seasoning
🥫 canned food | can tin
🍱 bento box | food
🍘 rice cracker | food
🍙 rice ball | food japanese onigiri
🍚 cooked rice | food bowl
🍛 curry rice | food
🍜 steaming bowl | chopsticks food noodle pho ramen soup noodles
🍝 spaghetti | food meatballs pasta restaurant
🍠 roasted sweet potato | food
🍢 oden | food kebab restaurant seafood skewer stick
🍣 sushi | food
🍤 fried shrimp | prawn tempura
🍥 fish cake with swirl | food pastry restaurant
🥮 moon cake | autumn festival yuèbǐng
🍡 dango | dessert japanese skewer stick sweet
🥟 dumpling | empanada gyōza jiaozi pierogi potsticker
🥠 fortune cookie | prophecy
🥡 takeout box | chopsticks delivery food oyster pail
🍦 soft ice cream | dessert food icecream restaurant serve sweet
🍧 shaved ice | dessert restaurant sweet
🍨 ice cream | dessert food restaurant sweet
🍩 doughnut | breakfast dessert donut food sweet
🍪 cookie | chip chocolate dessert sweet biscuit
🎂 birthday cake | bday celebration dessert happy pastry sweet
🍰 shortcake | cake dessert pastry slice sweet
🧁 cupcake | bakery dessert sprinkles sugar sweet treat
🥧 pie | apple filling fruit meat pastry pumpkin slice
🍫 chocolate bar | candy dessert halloween sweet tooth
🍬 candy | cavities dessert halloween restaurant sweet tooth wrapper
🍭 lollipop | candy dessert food restaurant sweet
🍮 custard | dessert pudding sweet flan
🍯 honey pot | barrel bear food honeypot jar sweet
🍼 baby bottle | babies birth born drink infant milk newborn
🥛 glass of milk | drink
☕ hot beverage | cafe caffeine chai coffee drink morning steaming tea
🫖 teapot | brew drink food pot tea
🍵 teacup without handle | beverage cup drink oolong tea green matcha
🍶 sake | bar beverage bottle cup drink restaurant
🍾 bottle with popping cork | bar drink champagne
🍷 wine glass | alcohol bar beverage booze club drink drinking drinks restaurant red
🍸 cocktail glass | alcohol bar booze club drink drinking drinks mad martini men
🍹 tropical drink | alcohol bar booze club cocktail drinking drinks drunk mai party tai tropics
🍺 beer mug | alcohol ale bar booze drink drinking drinks octoberfest oktoberfest pint stein summer
🍻 clinking beer mugs | alcohol bar booze bottoms cheers clink drinking drinks beers
🥂 clinking glasses | celebrate clink drink glass toast
🥃 tumbler glass | liquor scotch shot whiskey whisky
🫗 pouring liquid | accident drink empty glass oops pour spill water
🥤 cup with straw | drink juice malt soda soft water
🧋 bubble tea | boba food milk pearl
🧃 beverage box | juice straw sweet
🧉 mate | drink
🧊 ice | cold cube iceberg
🥢 chopsticks | hashi jeotgarak kuaizi
🍽 fork and knife with plate | cooking dinner eat
🍴 fork and knife | breakfast breaky cooking cutlery delicious dinner eat feed food hungry lunch restaurant yum yummy
🥄 spoon | eat tableware
🔪 kitchen knife | chef cooking hocho tool weapon
🫙 jar | condiment container empty nothing sauce store jam storage
🏺 amphora | aquarius cooking drink jug tool weapon zodiac
";

const HOME: &str = "
🏠 house | building country heart home ranch settle simple suburban suburbia where
🏡 house with garden | building country heart home ranch settle simple suburban suburbia where
👓 glasses | clothing eye eyeglasses eyewear
🕶 sunglasses | dark eye eyewear glasses
🥽 goggles | dive eye protection scuba swimming welding
🥼 lab coat | clothes doctor dr experiment jacket scientist white
🦺 safety vest | emergency
👔 necktie | clothing employed serious shirt tie
👕 t-shirt | blue casual clothes clothing collar dressed shirt shopping tshirt weekend
👖 jeans | blue casual clothes clothing denim dressed pants shopping trousers weekend
🧣 scarf | bundle cold neck up
🧤 gloves | hand
🧥 coat | brr bundle cold jacket up
🧦 socks | stocking
👗 dress | clothes clothing dressed fancy shopping
👘 kimono | clothing comfortable
🥻 sari | clothing dress
🩱 one-piece swimsuit | bathing suit
🩲 briefs | bathing one-piece suit swimsuit underwear
🩳 shorts | bathing pants suit swimsuit underwear
👙 bikini | bathing beach clothing pool suit swim
👚 woman's clothes | blouse clothing collar dress dressed lady shirt shopping woman
🪭 folding hand fan | clack clap cool cooling dance flirt flutter hot shy
👛 purse | clothes clothing coin dress fancy handbag shopping
👜 handbag | bag clothes clothing dress lady purse shopping
👝 clutch bag | clothes clothing dress handbag pouch purse
🛍 shopping bags | bag hotel
🎒 backpack | backpacking bag bookbag education rucksack satchel school
🩴 thong sandal | beach flip flop sandals shoe thongs zōri
👞 man's shoe | brown clothes clothing feet foot kick man shoes shopping
👟 running shoe | athletic clothes clothing fast kick shoes shopping sneaker tennis
🥾 hiking boot | backpacking brown camping outdoors shoe
🥿 flat shoe | ballet comfy flats slip-on slipper
👠 high-heeled shoe | clothes clothing dress fashion heel heels shoes shopping stiletto woman
👡 woman's sandal | clothing shoe woman
🩰 ballet shoes | dance
👢 woman's boot | clothes clothing dress shoe shoes shopping woman
🪮 hair pick | afro comb groom
👑 crown | clothing family king medieval queen royal royalty win
👒 woman's hat | clothes clothing garden hats party woman
🎩 top hat | clothes clothing fancy formal magic tophat
🎓 graduation cap | celebration clothing education hat scholar
🧢 billed cap | baseball bent dad hat
🪖 military helmet | army soldier war warrior
⛑ rescue worker's helmet | aid cross face hat
📿 prayer beads | clothing necklace religion
💄 lipstick | cosmetics date makeup
💍 ring | diamond engaged engagement married romance shiny sparkling wedding
💎 gem stone | diamond engagement jewel money romance wedding
🎥 movie camera | bollywood cinema film hollywood record
🎞 film frames | cinema movie
📽 film projector | cinema movie video
🎬 clapper board | action movie
📺 television | tv video
📷 camera | photo selfie snap tbt trip video
📸 camera with flash | video
📹 video camera | camcorder tbt
📼 videocassette | old school tape vcr vhs video
🔍 magnifying glass tilted left | lab left-pointing science search tool
🔎 magnifying glass tilted right | contact lab right-pointing science search tool
🕯 candle | light
💡 light bulb | comic electric idea
🔦 flashlight | electric light tool torch
🏮 red paper lantern | bar light restaurant
🪔 diya lamp | light oil
🚪 door | back closet front
🛗 elevator | accessibility hoist lift
🪞 mirror | makeup reflection reflector speculum
🪟 window | air frame fresh opening transparent view
🛏 bed | hotel sleep
🛋 couch and lamp | hotel sofa
🪑 chair | seat sit
🚽 toilet | bathroom
🪠 plunger | cup force plumber poop suction toilet
🚿 shower | water
🛁 bathtub | bath
🪤 mouse trap | bait cheese lure mousetrap snare
🪒 razor | sharp shave
🧴 lotion bottle | moisturizer shampoo sunscreen soap
🧷 safety pin | diaper punk rock
🧹 broom | cleaning sweeping witch sweep
🧺 basket | farming laundry picnic
🧻 roll of paper | toilet towels
🪣 bucket | cask pail vat
🧼 soap | bar bathing clean cleaning lather soapdish
🫧 bubbles | bubble burp clean floating pearl soap underwater
🪥 toothbrush | bathroom brush clean dental hygiene teeth toiletry
🧽 sponge | absorbing cleaning porous soak
🧯 fire extinguisher | extinguish quench
🛒 shopping cart | trolley
🚬 cigarette | smoking
⚰ coffin | dead death vampire
🪦 headstone | cemetery dead grave graveyard memorial rip tomb tombstone
⚱ funeral urn | ashes death
🧿 nazar amulet | bead blue charm evil-eye talisman
🪬 hamsa | amulet fatima fortune guide hand mary miriam palm protect protection
🗿 moai | face moyai statue stoneface travel
🪧 placard | card demonstration notice picket plaque protest sign
🪪 identification card | credentials document id license security
";

const OFFICE: &str = "
📱 mobile phone | cell communication telephone
📲 mobile phone with arrow | build call cell communication receive telephone
☎ telephone | phone
📞 telephone receiver | communication phone voip
📟 pager | communication
📠 fax machine | communication
🔋 battery
🪫 low battery | drained electronic energy power
🔌 electric plug | electricity
💻 laptop | computer office pc personal
🖥 desktop computer | monitor
🖨 printer | computer
⌨ keyboard | computer
🖱 computer mouse
🖲 trackball | computer
💽 computer disk | minidisk optical
💾 floppy disk | computer save
💿 optical disk | blu-ray cd computer dvd disc
📀 dvd | blu-ray cd computer disk optical
🧮 abacus | calculation calculator
📔 notebook with decorative cover | book decorated education school writing
📕 closed book | education red
📖 open book | education fantasy knowledge library novels reading
📗 green book | education fantasy library reading
📘 blue book | education fantasy library reading
📙 orange book | education fantasy library reading
📚 books | book education fantasy knowledge library novels reading school study
📓 notebook
📒 ledger | notebook
📃 page with curl | document paper
📜 scroll | paper
📄 page facing up | document paper
📰 newspaper | communication news paper
🗞 rolled-up newspaper | news paper rolled
📑 bookmark tabs | mark marker
🔖 bookmark | mark
🏷 label | tag price
🪙 coin | dollar euro gold metal money rich silver treasure
💰 money bag | bank bet billion cash cost dollar gold million moneybag paid paying pot rich win
🪎 treasure chest | gem gold jewels loot money prize silver valuables wealth
💴 yen banknote | bank bill currency money note
💵 dollar banknote | bank bill currency money note cash
💶 euro banknote | 100 bank bill currency money note rich
💷 pound banknote | bank bill billion cash currency money note pounds
💸 money with wings | bank banknote bill billion cash dollar fly million note pay
💳 credit card | bank cash charge money pay
🧾 receipt | accounting bookkeeping evidence invoice proof
💹 chart increasing with yen | bank currency graph growth market money rise trend upward
✉ envelope | e-mail email letter mail
📧 e-mail | email letter mail
📨 incoming envelope | delivering e-mail email letter mail receive sent
📩 envelope with arrow | communication down e-mail email letter mail outgoing send sent
📤 outbox tray | box email letter mail sent
📥 inbox tray | box email letter mail receive zero
📦 package | box communication delivery parcel shipping
📫 closed mailbox with raised flag | communication mail postbox
📪 closed mailbox with lowered flag | mail postbox
📬 open mailbox with raised flag | mail postbox
📭 open mailbox with lowered flag | mail postbox
📮 postbox | mail mailbox
🗳 ballot box with ballot
✏ pencil
✒ black nib | pen
🖋 fountain pen
🖊 pen | ballpoint
🖌 paintbrush | painting
🖍 crayon
📝 memo | communication media notes pencil note
🪌 eraser | correction delete drawing erase error mistake remove rub undo writing
💼 briefcase | office
📁 file folder
📂 open file folder
🗂 card index dividers
📅 calendar | date
📆 tear-off calendar
🗒 spiral notepad | note pad
🗓 spiral calendar | pad
📇 card index | old rolodex school
📈 chart increasing | data graph growth right trend up upward
📉 chart decreasing | data down downward graph negative trend
📊 bar chart | data graph
📋 clipboard | do list notes
📌 pushpin | collage pin
📍 round pushpin | location map pin
📎 paperclip
🖇 linked paperclips | link paperclip
📏 straight ruler | angle edge math straightedge
📐 triangular ruler | angle math rule set slide triangle
✂ scissors | cut cutting paper tool
🗃 card file box
🗄 file cabinet | filing paper
🗑 wastebasket | can garbage trash waste bin
🔒 locked | closed lock private padlock
🔓 unlocked | cracked lock open unlock
🔏 locked with pen | ink lock nib privacy
🔐 locked with key | bike closed lock secure
🔑 key | keys lock major password unlock
🗝 old key | clue lock
🪍 net with handle | catch collect mesh trap
🔨 hammer | home improvement repairs tool
🪓 axe | ax chop hatchet split wood
⛏ pick | hammer mining tool
⚒ hammer and pick | tool
🛠 hammer and wrench | spanner tool repair
🗡 dagger | knife weapon
⚔ crossed swords | weapon
💣 bomb | boom comic dangerous explosion hot
🪃 boomerang | rebound repercussion weapon
🏹 bow and arrow | archer archery sagittarius tool weapon zodiac
🛡 shield | weapon
🪚 carpentry saw | carpenter cut lumber tool trim
🔧 wrench | home improvement spanner tool
🪛 screwdriver | flathead handy screw tool
🔩 nut and bolt | home improvement tool screw
⚙ gear | cog cogwheel tool settings
🗜 clamp | compress tool vice
⚖ balance scale | justice libra scales tool weight zodiac
🦯 white cane | accessibility blind probing
🔗 link | links chain
⛓ chains | chain
🪝 hook | catch crook curve ensnare point selling
🧰 toolbox | box chest mechanic red tool
🧲 magnet | attraction horseshoe magnetic negative positive shape u
🪜 ladder | climb rung step
🪏 shovel | bury dig garden hole plant scoop snow spade
⚗ alembic | chemistry tool
🧪 test tube | chemist chemistry experiment lab science
🧫 petri dish | bacteria biologist biology culture lab
🧬 dna | biologist evolution gene genetics life
🔬 microscope | experiment lab science tool
🔭 telescope | contact extraterrestrial science tool
📡 satellite antenna | aliens contact dish science
💉 syringe | doctor flu medicine needle shot sick tool vaccination injection
🩸 drop of blood | bleed donation injury medicine menstruation
💊 pill | doctor drugs medicated medicine pills sick vitamin
🩹 adhesive bandage | plaster
🩼 crutch | aid cane disability help hurt injured mobility stick
🩺 stethoscope | doctor heart medicine
🩻 x-ray | bones doctor medical skeleton skull xray
";

const NATURE: &str = "
🐵 monkey face | animal banana
🐒 monkey | animal banana
🦍 gorilla | animal
🦧 orangutan | animal ape monkey
🐶 dog face | adorbs animal pet puppies puppy
🐕 dog | animal animals dogs pet
🦮 guide dog | accessibility animal blind
🐩 poodle | animal dog fluffy
🐺 wolf | animal face
🦊 fox | animal face
🦝 raccoon | animal curious sly
🐱 cat face | animal kitten kitty pet
🐈 cat | animal animals cats kitten pet
🦁 lion | alpha animal face leo mane order rawr roar safari strong zodiac
🐯 tiger face | animal big cat predator
🐅 tiger | animal big cat predator zoo
🐆 leopard | animal big cat predator zoo
🐴 horse face | animal dressage equine farm horses
🫎 moose | alces animal antlers elk mammal
🫏 donkey | animal ass burro hinny mammal mule stubborn
🐎 horse | animal equestrian farm racehorse racing
🦄 unicorn | face
🦓 zebra | animal stripe
🦌 deer | animal
🦬 bison | animal buffalo herd wisent
🐮 cow face | animal farm milk moo
🐂 ox | animal animals bull farm taurus zodiac
🐃 water buffalo | animal zoo
🐄 cow | animal animals farm milk moo
🐷 pig face | animal bacon farm pork
🐖 pig | animal bacon farm pork sow
🐗 boar | animal pig
🐽 pig nose | animal face farm smell snout
🐏 ram | animal aries horns male sheep zodiac zoo
🐑 ewe | animal baa farm female fluffy lamb sheep wool
🐐 goat | animal capricorn farm milk zodiac
🐪 camel | animal desert dromedary hump one
🐫 two-hump camel | animal bactrian desert hump two
🦙 llama | alpaca animal guanaco vicuña wool
🦒 giraffe | animal spots
🐘 elephant | animal
🦣 mammoth | animal extinction large tusk wooly
🦏 rhinoceros | animal
🦛 hippopotamus | animal hippo
🐭 mouse face | animal
🐁 mouse | animal animals
🐀 rat | animal
🐹 hamster | animal face pet
🐰 rabbit face | animal bunny pet
🐇 rabbit | animal bunny pet
🐿 chipmunk | animal squirrel
🦫 beaver | animal dam teeth
🦔 hedgehog | animal spiny
🦇 bat | animal vampire
🐻 bear | animal face grizzly growl honey
🐨 koala | animal australia bear down face marsupial under
🐼 panda | animal bamboo face
🦥 sloth | lazy slow
🦦 otter | animal fishing playful
🦨 skunk | animal stink
🦘 kangaroo | animal joey jump marsupial
🦡 badger | animal honey pester
🐾 paw prints | feet paws print pet
🦃 turkey | bird gobble thanksgiving
🐔 chicken | animal bird ornithology
🐓 rooster | animal bird ornithology
🐣 hatching chick | animal baby bird egg
🐤 baby chick | animal bird ornithology
🐥 front-facing baby chick | animal bird newborn ornithology
🐦 bird | animal ornithology
🐧 penguin | animal antarctica bird ornithology
🕊 dove | bird fly ornithology peace
🦅 eagle | animal bird ornithology
🦆 duck | animal bird ornithology
🦢 swan | animal bird cygnet duckling ornithology ugly
🦉 owl | animal bird ornithology wise
🦤 dodo | animal bird extinction large ornithology
🪶 feather | bird flight light plumage
🦩 flamingo | animal bird flamboyant ornithology tropical
🦚 peacock | animal bird colorful ornithology ostentatious peahen pretty proud
🦜 parrot | animal bird ornithology pirate talk
🪽 wing | angelic ascend aviation bird fly flying heavenly mythology soar
🪿 goose | animal bird duck flock fowl gaggle gander geese honk ornithology silly
🐸 frog | animal face
🐊 crocodile | animal zoo
🐢 turtle | animal terrapin tortoise
🦎 lizard | animal reptile
🐍 snake | animal bearer ophiuchus serpent zodiac
🐲 dragon face | animal fairy fairytale tale
🐉 dragon | animal fairy fairytale knights tale
🦕 sauropod | brachiosaurus brontosaurus dinosaur diplodocus
🦖 t-rex | dinosaur rex t tyrannosaurus
🐳 spouting whale | animal beach face ocean
🐋 whale | animal beach ocean
🐬 dolphin | animal beach flipper ocean
🫍 orca | marine ocean whale
🦭 seal | animal lion ocean sea
🐟 fish | animal dinner fishes fishing pisces zodiac
🐠 tropical fish | animal fishes
🐡 blowfish | animal fish
🦈 shark | animal fish
🐙 octopus | animal creature ocean
🐚 spiral shell | animal beach conch sea
🪸 coral | change climate ocean reef sea
🪼 jellyfish | animal aquarium burn invertebrate jelly life marine ocean ouch plankton sea sting stinger tentacles
🦀 crab | cancer zodiac
🦞 lobster | animal bisque claws seafood
🦐 shrimp | food shellfish small
🦑 squid | animal food mollusk
🦪 oyster | diving pearl
🐌 snail | animal escargot garden nature slug
🦋 butterfly | insect pretty
🫌 monarch butterfly | caterpillar change fragile hope insect metamorphosis migration transformation
🐛 bug | animal garden insect caterpillar
🐜 ant | animal garden insect
🐝 honeybee | animal bee bumblebee honey insect nature spring
🪲 beetle | animal bug insect
🐞 lady beetle | animal garden insect ladybird ladybug nature
🦗 cricket | animal bug grasshopper insect orthoptera
🪳 cockroach | animal insect pest roach
🕷 spider | animal insect
🕸 spider web
🦂 scorpion | scorpio scorpius zodiac
🦟 mosquito | bite disease fever insect malaria pest virus
🪰 fly | animal disease insect maggot pest rotting
🪱 worm | animal annelid earthworm parasite
🦠 microbe | amoeba bacteria science virus
💐 bouquet | anniversary birthday date flower love plant romance flowers
🌸 cherry blossom | flower plant spring springtime
💮 white flower
🪷 lotus | beauty buddhism calm flower hinduism peace purity serenity
🏵 rosette | plant
🌹 rose | beauty elegant flower love plant red valentine
🥀 wilted flower | dying
🌺 hibiscus | flower plant
🌻 sunflower | flower outdoors plant sun
🌼 blossom | buttercup dandelion flower plant daisy
🌷 tulip | blossom flower growth plant
🪻 hyacinth | bloom bluebonnet flower indigo lavender lilac lupine plant purple shrub snapdragon spring violet
🌱 seedling | plant sapling sprout young
🪴 potted plant | decor grow house nurturing pot
🌲 evergreen tree | christmas forest pine
🌳 deciduous tree | forest green habitat shedding
🌴 palm tree | beach plant tropical
🌵 cactus | desert drought nature plant
🌾 sheaf of rice | ear grain grains plant
🌿 herb | leaf plant
☘ shamrock | irish plant
🍀 four leaf clover | 4 four-leaf irish lucky plant luck
🍁 maple leaf | falling autumn
🍂 fallen leaf | autumn fall falling
🍃 leaf fluttering in wind | blow flutter
🪹 empty nest | branch home nesting
🪺 nest with eggs | bird branch egg nesting
🍄 mushroom | fungus toadstool
🪾 leafless tree | bare barren branches dead drought trunk winter wood
🌑 new moon | dark space
🌒 waxing crescent moon | dreams space
🌓 first quarter moon | space
🌔 waxing gibbous moon | space
🌕 full moon | space
🌖 waning gibbous moon | space
🌗 last quarter moon | space
🌘 waning crescent moon | space
🌙 crescent moon | ramadan space night
🌚 new moon face | space
🌛 first quarter moon face | space
🌜 last quarter moon face | dreams
🌡 thermometer | weather
☀ sun | bright rays space sunny weather
🌝 full moon face | bright
🌞 sun with face | beach bright day heat shine sunny sunshine weather
🪐 ringed planet | saturn saturnine
⭐ star | astronomy medium stars white
🌟 glowing star | glittery glow night shining sparkle win
🌠 shooting star | falling night space
🌌 milky way | space
☁ cloud | weather
⛅ sun behind cloud | cloudy weather
⛈ cloud with lightning and rain | thunder thunderstorm
🌤 sun behind small cloud | weather
🌥 sun behind large cloud | weather
🌦 sun behind rain cloud | weather
🌧 cloud with rain | weather
🌨 cloud with snow | cold weather
🌩 cloud with lightning | weather
🌪 tornado | cloud weather whirlwind
🌫 fog | cloud weather
🌬 wind face | blow cloud
🌀 cyclone | dizzy hurricane twister typhoon weather
🌈 rainbow | gay genderqueer glbt glbtq lesbian lgbt lgbtq lgbtqia nature pride queer rain trans transgender weather
🌂 closed umbrella | clothing rain
☂ umbrella | clothing rain
☔ umbrella with rain drops | clothing drop weather
⛱ umbrella on ground | rain sun
⚡ high voltage | danger electric electricity lightning nature thunder thunderbolt zap bolt
❄ snowflake | cold snow weather
☃ snowman | cold man snow
⛄ snowman without snow | cold man
☄ comet | space
🪋 meteor | apocalypse falling fireball gravity rock space star
🔥 fire | af burn flame hot lit litaf tool
💧 droplet | cold comic drop nature sad sweat tear water weather
🌊 water wave | nature ocean surf surfer surfing sea
";

const FACES: &str = "
😀 grinning face | cheerful cheery grin happy laugh nice smile smiling teeth
😃 grinning face with big eyes | awesome grin happy mouth open smile smiling teeth yay
😄 grinning face with smiling eyes | eye grin happy laugh lol mouth open smile
😁 beaming face with smiling eyes | eye grin grinning happy nice smile teeth
😆 grinning squinting face | closed eyes haha hahaha happy laugh lol mouth open rofl smile smiling laughing
😅 grinning face with sweat | cold dejected excited mouth nervous open smile smiling stress stressed
🤣 rolling on the floor laughing | crying face funny haha happy hehe hilarious joy laugh lmao lol rofl roflmao tear
😂 face with tears of joy | crying feels funny haha happy hehe hilarious laugh lmao lol rofl roflmao tear
🙂 slightly smiling face | happy smile slight
🙃 upside-down face | hehe smile
🫠 melting face | disappear dissolve embarrassed haha heat hot liquid lol melt sarcasm sarcastic
🫫 cracking face | breaking crumbling masking resilience reveal tension transformation
😉 winking face | flirt heartbreaker sexy slide tease wink winks
😊 smiling face with smiling eyes | blush eye glad satisfied smile
😇 smiling face with halo | angel angelic angels blessed fairy fairytale fantasy happy innocent peaceful smile spirit tale
🥰 smiling face with hearts | 3 adore crush heart ily love romance smile you
😍 smiling face with heart-eyes | 143 bae eye feels hearts ily kisses love romance romantic smile xoxo heart eyes
🤩 star-struck | excited eyes face grinning smile star starry-eyed wow struck
😘 face blowing a kiss | adorbs bae flirt heart ily love lover miss muah romantic smooch xoxo you
😗 kissing face | 143 date dating flirt ily kiss love smooch smooches xoxo you
☺ smiling face | happy outlined relaxed smile
😚 kissing face with closed eyes | 143 bae blush date dating eye flirt ily kisses smooches xoxo
😙 kissing face with smiling eyes | 143 closed date dating eye flirt ily kiss kisses love night smile
🥲 smiling face with tear | glad grateful happy joy pain proud relieved smile smiley touched
😋 face savoring food | delicious eat full hungry savor smile smiling tasty um yum yummy
😛 face with tongue | awesome cool nice party stuck-out sweet
😜 winking face with tongue | crazy epic eye funny joke loopy nutty party stuck-out wacky weirdo wink yolo
🤪 zany face | crazy eye eyes goofy large small
😝 squinting face with tongue | closed eye eyes gross horrible omg stuck-out taste whatever yolo
🤑 money-mouth face | money mouth paid
🤗 smiling face with open hands | hug hugging
🤭 face with hand over mouth | giggle giggling oops realization secret shock sudden surprise whoops
🫢 face with open eyes and hand over mouth | amazement awe disbelief embarrass gasp omg quiet scared shock surprise
🫣 face with peeking eye | captivated embarrass hide hiding peek peep scared shy stare
🤫 shushing face | quiet shh shush
🤔 thinking face | chin consider hmm ponder pondering wondering
🫡 saluting face | good luck ma'am ok respect salute sir troops yes
🤐 zipper-mouth face | keep mouth quiet secret shut zip zipper
🤨 face with raised eyebrow | disapproval disbelief distrust emoji hmm mild skeptic skeptical skepticism surprise what
😐 neutral face | awkward blank deadpan expressionless fine jealous meh oh shade straight unamused unhappy unimpressed whatever
😑 expressionless face | awkward dead fine inexpressive jealous meh not oh omg straight uh unhappy unimpressed whatever
😶 face without mouth | awkward blank expressionless mouthless mute quiet secret silence silent speechless
🫥 dotted line face | depressed disappear hidden hide introvert invisible meh whatever wtv
😏 smirking face | boss dapper flirt homie kidding leer shade slick sly smirk smug snicker suave suspicious swag
😒 unamused face | ... bored fine jealous jel jelly pissed smh ugh uhh unhappy weird whatever
🙄 face with rolling eyes | eyeroll shade ugh whatever eye roll
😬 grimacing face | awk awkward dentist grimace grinning smile smiling
🤥 lying face | liar lie pinocchio
🫨 shaking face | crazy daze earthquake omg panic shock surprise vibrate whoa wow
😌 relieved face | calm peace relief zen
😔 pensive face | awful bored dejected died disappointed losing lost sad sucks
😪 sleepy face | crying good night sad sleep sleeping tired
🤤 drooling face
😴 sleeping face | bed bedtime good goodnight nap night sleep tired whatever yawn zzz
🫩 face with bags under eyes | bored exhausted fatigued late sleepy tired weary
😷 face with medical mask | cold dentist dermatologist doctor dr germs medicine sick
🤒 face with thermometer | ill sick fever
🤕 face with head-bandage | bandage hurt injury ouch
🤢 nauseated face | gross nasty sick vomit
🤮 face vomiting | barf ew gross puke sick spew throw up vomit
🤧 sneezing face | fever flu gesundheit sick sneeze
🥵 hot face | dying feverish heat panting red-faced stroke sweating tongue
🥶 cold face | blue blue-faced freezing frostbite icicles subzero teeth
🥴 woozy face | dizzy drunk eyes intoxicated mouth tipsy uneven wavy
😵 face with crossed-out eyes | dead dizzy feels knocked out sick tired
🤯 exploding head | blown explode mind mindblown no shocked way
🤠 cowboy hat face | cowgirl
🥳 partying face | bday birthday celebrate celebration excited happy hat hooray horn party
🥸 disguised face | disguise eyebrow glasses incognito moustache mustache nose person spy tache tash
😎 smiling face with sunglasses | awesome beach bright bro chilling cool rad relaxed shades slay smile style swag win
🤓 nerd face | brainy clever expert geek gifted glasses intelligent smart
🧐 face with monocle | classy fancy rich stuffy wealthy
😕 confused face | befuddled confusing dunno frown hm meh not sad sorry sure
🫤 face with diagonal mouth | confused confusion disappointed doubt doubtful frustrated frustration meh skeptical unsure whatever wtv
😟 worried face | anxious butterflies nerves nervous sad stress stressed surprised worry
🙁 slightly frowning face | frown sad
☹ frowning face | frown sad
😮 face with open mouth | believe forgot omg shocked surprised sympathy unbelievable unreal whoa wow you
😯 hushed face | epic omg stunned surprised whoa woah
😲 astonished face | cost no omg shocked totally way
😳 flushed face | amazed awkward crazy dazed dead disbelief embarrassed geez heat hot impressed jeez what wow
🫪 distorted face | anxiety bloated panic shocked surprised vulnerable
🥺 pleading face | begging big eyes mercy not please pretty puppy sad why
🥹 face holding back tears | admiration aww cry embarrassed feelings grateful gratitude joy please proud resist sad
😦 frowning face with open mouth | caught frown guard scared scary surprise what wow
😧 anguished face | forgot scared scary stressed surprise unhappy what wow
😨 fearful face | afraid anxious blame fear scared worried
😰 anxious face with sweat | blue cold eek mouth nervous open rushed scared yikes
😥 sad but relieved face | anxious call close complicated disappointed not sweat time whew
😢 crying face | awful cry feels miss sad tear triste unhappy
😭 loudly crying face | bawling cry sad sob tear tears unhappy sobbing
😱 face screaming in fear | epic fearful munch scared scream screamer shocked surprised woah
😖 confounded face | annoyed confused cringe distraught feels frustrated mad sad
😣 persevering face | concentrate concentration focus headache persevere
😞 disappointed face | awful blame dejected fail losing sad unhappy
😓 downcast face with sweat | close cold feels headache nervous sad scared yikes
😩 weary face | crying fail feels hungry mad nooo sad sleepy tired unhappy
😫 tired face | cost feels nap sad sneeze
🥱 yawning face | bedtime bored goodnight nap night sleep sleepy tired whatever yawn zzz
😤 face with steam from nose | anger angry feels fume fuming furious fury mad triumph unhappy won
😡 enraged face | anger angry feels mad maddening pouting rage red shade unhappy upset
😠 angry face | anger blame feels frustrated mad maddening rage shade unhappy upset
🤬 face with symbols on mouth | censor cursing cussing mad pissed swearing
😈 smiling face with horns | demon devil evil fairy fairytale fantasy purple shade smile tale
👿 angry face with horns | demon devil evil fairy fairytale fantasy imp mischievous purple shade tale
💀 skull | body dead death face fairy fairytale i'm lmao monster tale yolo
☠ skull and crossbones | bone dead death face monster poison
💩 pile of poo | bs comic doo dung face fml monster poop smelly smh stink stinks stinky turd
🤡 clown face
👹 ogre | creature devil face fairy fairytale fantasy mask monster scary tale
👺 goblin | angry creature face fairy fairytale fantasy mask mean monster tale
👻 ghost | boo creature excited face fairy fairytale fantasy halloween haunting monster scary silly tale
👽 alien | creature extraterrestrial face fairy fairytale fantasy monster space tale ufo
👾 alien monster | creature extraterrestrial face fairy fairytale fantasy game gamer games pixelated space tale ufo
🤖 robot | face monster
😺 grinning cat | animal face mouth open smile smiling
😸 grinning cat with smiling eyes | animal eye face grin smile
😹 cat with tears of joy | animal face laugh laughing lol tear
😻 smiling cat with heart-eyes | animal eye face heart love smile
😼 cat with wry smile | animal face ironic
😽 kissing cat | animal closed eye eyes face kiss
🙀 weary cat | animal face oh surprised
😿 crying cat | animal cry face sad tear
😾 pouting cat | animal face
🙈 see-no-evil monkey | embarrassed evil face forbidden forgot gesture hide no omg prohibited scared secret smh watch
🙉 hear-no-evil monkey | animal ears evil face forbidden gesture hear listen no not prohibited secret shh tmi
🙊 speak-no-evil monkey | animal evil face forbidden gesture no not oops prohibited quiet secret speak stealth
💌 love letter | heart mail romance valentine
💘 heart with arrow | 143 adorbs cupid date emotion ily love romance valentine
💝 heart with ribbon | 143 anniversary emotion ily kisses valentine xoxo
💖 sparkling heart | 143 emotion excited good ily kisses morning night sparkle xoxo
💗 growing heart | 143 emotion excited heartpulse ily kisses muah nervous pulse xoxo
💓 beating heart | 143 cardio emotion heartbeat ily love pulsating pulse
💞 revolving hearts | 143 adorbs anniversary emotion heart
💕 two hearts | 143 anniversary date dating emotion heart ily kisses love loving xoxo
💟 heart decoration | 143 emotion hearth purple white
❣ heart exclamation | bang heavy mark punctuation
💔 broken heart | break crushed emotion heartbroken lonely sad
❤ red heart | emotion love
🩷 pink heart | 143 adorable cute emotion ily like love special sweet
🧡 orange heart | 143
💛 yellow heart | 143 cardiac emotion ily love
💚 green heart | 143 emotion ily love romantic
💙 blue heart | 143 emotion ily love romance
🩵 light blue heart | 143 cute cyan emotion ily like love sky special teal
💜 purple heart | 143 bestest emotion ily love
🤎 brown heart | 143
🖤 black heart | evil wicked
🩶 grey heart | 143 emotion gray ily love silver slate special
🤍 white heart | 143
💋 kiss mark | dating emotion heart kissing lips romance sexy
💯 hundred points | 100 a+ agree clearly definitely faithful fleek full keep perfect point score true truth yup
💢 anger symbol | angry comic mad upset
🫯 fight cloud | argument brawl debate disagreement ruckus wrestle
💥 collision | bomb boom collide comic explode
💫 dizzy | comic shining shooting star stars
💦 sweat droplets | comic drip droplet drops splashing squirt water wet work workout
💨 dashing away | cloud comic dash fart fast go gone gotta running smoke
🕳 hole
💬 speech balloon | bubble comic dialog message sms talk text typing
🗨 left speech bubble | balloon dialog
🗯 right anger bubble | angry balloon mad
💭 thought balloon | bubble cartoon cloud comic daydream decisions dream idea invent invention realize think thoughts wonder
💤 zzz | comic good goodnight night sleep sleeping sleepy tired
";

const PEOPLE: &str = "
👋 waving hand | bye cya g2g greetings gtg hello hey hi later outtie ttfn ttyl wave yo you
🤚 raised back of hand | backhand
🖐 hand with fingers splayed | finger raised stop
✋ raised hand | 5 five high stop
🖖 vulcan salute | finger hand hands
🫱 rightwards hand | handshake hold reach right rightward shake
🫲 leftwards hand | handshake hold left leftward reach shake
🫳 palm down hand | dismiss drop dropped pick shoo up
🫴 palm up hand | beckon catch come hold know lift me offer tell
🫷 leftwards pushing hand | block five halt high hold leftward pause push refuse slap stop wait
🫸 rightwards pushing hand | block five halt high hold pause push refuse rightward slap stop wait
👌 ok hand | awesome bet dope fleek fosho got gotcha legit okay pinch rad sure sweet three
🤌 pinched fingers | gesture hand hold huh interrogation patience relax sarcastic ugh what zip
🤏 pinching hand | amount bit fingers little small sort
✌ victory hand | peace v
🤞 crossed fingers | cross finger hand luck
🫰 hand with index finger and thumb crossed | <3 expensive heart love money snap
🤟 love-you gesture | fingers hand ily love three you
🤘 sign of the horns | finger hand rock-on
🤙 call me hand | hang loose shaka
👈 backhand index pointing left | finger hand point
👉 backhand index pointing right | finger hand point
👆 backhand index pointing up | finger hand point
🖕 middle finger | hand
👇 backhand index pointing down | finger hand point
☝ index pointing up | finger hand point this
🫵 index pointing at the viewer | finger hand poke you
👍 thumbs up | +1 good hand like thumb yes
👎 thumbs down | -1 bad dislike good hand no nope thumb
🫹 leftwards thumb sign | grip left neutral point that this
🫺 rightwards thumb sign | grip neutral point right that this
✊ raised fist | clenched hand punch solidarity
👊 oncoming fist | absolutely agree boom bro bruh bump clenched correct hand knuckle pound punch rock ttyl
🤛 left-facing fist | leftwards
🤜 right-facing fist | rightwards
👏 clapping hands | applause approval awesome clap congrats congratulations excited good great hand homie job nice prayed well yay
🙌 raising hands | celebration gesture hand hooray praise raised
🫶 heart hands | <3 love you
👐 open hands | hand hug jazz swerve
🤲 palms up together | cupped dua hands pray prayer wish
🤝 handshake | agreement deal hand meeting shake
🙏 folded hands | appreciate ask beg blessed bow cmon five gesture hand high please pray thanks thx
✍ writing hand | write
💅 nail polish | bored care cosmetics done makeup manicure whatever
🤳 selfie | camera phone
💪 flexed biceps | arm beast bench bodybuilder bro curls flex gains gym jacked muscle press ripped strong weightlift
🦾 mechanical arm | accessibility prosthetic
🦿 mechanical leg | accessibility prosthetic
🦵 leg | bent foot kick knee limb
🦶 foot | ankle feet kick stomp
👂 ear | body ears hear hearing listen listening sound
🦻 ear with hearing aid | accessibility hard
👃 nose | body noses nosey odor smell smells
🧠 brain | intelligent smart
🫀 anatomical heart | beat cardiology heartbeat organ pulse real red
🫁 lungs | breath breathe exhalation inhalation lung organ respiration
🦷 tooth | dentist pearly teeth white
🦴 bone | bones dog skeleton wishbone
👀 eyes | body eye face googly look looking omg peep see seeing
👁 eye | 1 body one
👅 tongue | body lick slurp
👄 mouth | beauty body kiss kissing lips lipstick
🫦 biting lip | anxious bite fear flirt flirting kiss lipstick nervous sexy uncomfortable worried worry
👶 baby | babies children goo infant newborn pregnant young
🧒 child | bright-eyed grandchild kid young younger
👦 boy | bright-eyed child grandson kid son young younger
👧 girl | bright-eyed child daughter granddaughter kid virgo young younger zodiac
🧑 person | adult
👱 person: blond hair | blond-haired human person
👨 man | adult bro
🧔 person: beard | bearded person whiskers
👩 woman | adult lady
🧓 older person | adult elderly grandparent old wise
👴 old man | adult bald elderly gramps grandfather grandpa wise
👵 old woman | adult elderly grandma grandmother granny lady wise
🙍 person frowning | annoyed disappointed disgruntled disturbed frown frustrated gesture irritated upset
🙎 person pouting | disappointed downtrodden frown grimace scowl sulk upset whine
🙅 person gesturing no | forbidden gesture hand not prohibit
🙆 person gesturing ok | exercise gesture hand omg
💁 person tipping hand | fetch flick flip gossip sarcasm sarcastic sassy seriously whatever
🙋 person raising hand | gesture here know me pick question raise
🧏 deaf person | accessibility ear gesture hear
🙇 person bowing | apology ask beg bow favor forgive gesture meditate meditation pity regret sorry
🤦 person facepalming | again bewilder disbelief exasperation facepalm no not oh omg shock smh
🤷 person shrugging | doubt dunno guess idk ignorance indifference knows maybe shrug whatever who
👮 police officer | apprehend arrest citation cop law over pulled undercover
🕵 detective | sleuth spy
💂 guard | buckingham helmet london palace
🥷 ninja | assassin fight fighter hidden person secret skills sly soldier stealth war
👷 construction worker | build fix hardhat hat man person rebuild remodel repair work
🫅 person with crown | monarch noble regal royal royalty
🤴 prince | crown fairy fairytale fantasy king royal royalty tale
👸 princess | crown fairy fairytale fantasy queen royal royalty tale
👳 person wearing turban
👲 person with skullcap | cap chinese gua guapi hat mao pi
🧕 woman with headscarf | bandana head hijab kerchief mantilla tichel
🤵 person in tuxedo | formal wedding
👰 person with veil | wedding
🤰 pregnant woman
🫃 pregnant man | belly bloated full overeat
🫄 pregnant person | belly bloated full overeat stuffed
🤱 breast-feeding | baby breast feeding mom mother nursing woman
👼 baby angel | church face fairy fairytale fantasy tale
🎅 santa claus | celebration christmas fairy fantasy father holiday merry tale xmas
🤶 mrs. claus | celebration christmas fairy fantasy holiday merry mother mrs santa tale xmas
🦸 superhero | good hero superpower
🦹 supervillain | bad criminal evil superpower villain
🧙 mage | fantasy magic play sorcerer sorceress sorcery spell summon witch wizard
🧚 fairy | fairytale fantasy myth person pixie tale wings
🧛 vampire | blood dracula fangs halloween scary supernatural teeth undead
🧜 merperson | creature fairytale folklore ocean sea siren trident
🧝 elf | elves enchantment fantasy folklore magic magical myth
🧞 genie | djinn fantasy jinn lamp myth rub wishes
🧟 zombie | apocalypse dead halloween horror scary undead walking
🧌 troll | fairy fantasy monster tale trolling
🫈 hairy creature | bigfoot cryptid forest giant sasquatch woodwose yeti
💆 person getting massage | face headache relax relaxing salon soothe spa tension therapy treatment
💇 person getting haircut | barber beauty chop cosmetology cut groom hair parlor shears style
🚶 person walking | amble gait hike man pace pedestrian stride stroll walk
🧍 person standing | stand
🧎 person kneeling | kneel knees
🏃 person running | fast hurry marathon move quick race racing run rush speed
💃 woman dancing | dance dancer elegant festive flair flamenco groove let's salsa tango
🕺 man dancing | dance dancer elegant festive flair flamenco groove let's salsa tango
🕴 person in suit levitating | business
👯 people with bunny ears | bestie bff counterpart dancer double ear identical pair party partying soulmate twin twinsies
🧖 person in steamy room | day luxurious pamper relax sauna spa steam steambath unwind
🧗 person climbing | climb climber mountain rock scale up
🤺 person fencing | fencer sword
🏇 horse racing | jockey racehorse riding sport
⛷ skier | ski snow
🏂 snowboarder | ski snow snowboard sport
🏌 person golfing | ball birdie caddy driving golf green pga putt range tee
🏄 person surfing | beach ocean sport surf surfer swell waves
🚣 person rowing boat | canoe cruise fishing lake oar paddle raft river row rowboat
🏊 person swimming | freestyle sport swim swimmer triathlon
⛹ person bouncing ball | athletic basketball championship dribble net player throw
🏋 person lifting weights | barbell bodybuilder deadlift lifter powerlifting weight weightlifter workout
🚴 person biking | bicycle bicyclist bike cycle cyclist riding sport
🚵 person mountain biking | bicycle bicyclist bike cycle cyclist riding sport
🤸 person cartwheeling | active cartwheel excited flip gymnastics happy somersault
🤼 people wrestling | combat duel grapple ring tournament wrestle
🤽 person playing water polo | sport swimming waterpolo
🤾 person playing handball | athletics ball catch chuck hurl lob pitch sport throw toss
🤹 person juggling | act balance balancing handle juggle manage multitask skill
🧘 person in lotus position | cross legged legs meditation peace relax serenity yoga yogi zen
🛀 person taking bath | bathtub tub
🛌 person in bed | bedtime good goodnight hotel nap night sleep tired zzz
👭 women holding hands | bae bestie bff couple dating flirt friends girls hand hold sisters twins
👫 woman and man holding hands | bae bestie bff couple dating flirt friends hand hold twins
👬 men holding hands | bae bestie bff boys brothers couple dating flirt friends hand hold twins
💏 kiss | anniversary babe bae couple date dating heart love mwah person romance together xoxo
💑 couple with heart | anniversary babe bae dating kiss love person relationship romance together you
🗣 speaking head | face silhouette speak
👤 bust in silhouette | mysterious shadow
👥 busts in silhouette | bff bust everyone friend friends people
🫂 people hugging | comfort embrace farewell friendship goodbye hello hug love thanks
👪 family | child
👣 footprints | barefoot clothing footprint omw print walk
🫆 fingerprint | clue crime detective forensics identity mystery print safety trace
";

const TRAVEL: &str = "
🌍 globe showing europe-africa | africa earth europe world
🌎 globe showing americas | earth world
🌏 globe showing asia-australia | asia australia earth world
🌐 globe with meridians | earth internet web world worldwide
🗺 world map
🗾 map of japan
🧭 compass | direction magnetic navigation orienteering
🏔 snow-capped mountain | cold snow
⛰ mountain
🛘 landslide | avalanche danger disaster earthquake mountain mudslide rocks
🌋 volcano | eruption mountain nature
🗻 mount fuji | mountain nature
🏕 camping
🏖 beach with umbrella
🏜 desert
🏝 desert island
🏞 national park
🏟 stadium
🏛 classical building
🏗 building construction | crane
🧱 brick | bricks clay mortar wall
🪨 rock | boulder heavy solid stone tough
🪵 wood | log lumber timber
🛖 hut | home house roundhouse shelter yurt
🏘 houses | house
🏚 derelict house | home
🏢 office building | city cubical job
🏣 japanese post office | building
🏤 post office | building european
🏥 hospital | building doctor medicine
🏦 bank | building
🏨 hotel | building
🏩 love hotel | building
🏪 convenience store | 24 building hours shop
🏫 school | building
🏬 department store | building
🏭 factory | building
🏯 japanese castle | building
🏰 castle | building european
💒 wedding | chapel hitched nuptials romance
🗼 tokyo tower
🗽 statue of liberty | new ny nyc york
⛪ church | bless chapel christian cross religion
🕌 mosque | islam masjid muslim religion
🛕 hindu temple
🕍 synagogue | jew jewish judaism religion temple
⛩ shinto shrine | religion
🕋 kaaba | hajj islam muslim religion umrah
⛲ fountain
⛺ tent | camping
🌁 foggy | fog
🌃 night with stars | star
🏙 cityscape | city
🌄 sunrise over mountains | morning sun
🌅 sunrise | morning nature sun
🌆 cityscape at dusk | building city evening landscape sun sunset
🌇 sunset | building dusk sun
🌉 bridge at night
♨ hot springs | hotsprings steaming
🎠 carousel horse | entertainment
🛝 playground slide | amusement park play playing sliding theme
🎡 ferris wheel | amusement park theme
🎢 roller coaster | amusement park theme
💈 barber pole | cut fresh haircut shave
🎪 circus tent
🚂 locomotive | caboose engine railway steam train trains travel
🚃 railway car | electric train tram travel trolleybus
🚄 high-speed train | railway shinkansen speed
🚅 bullet train | high-speed nose railway shinkansen speed travel
🚆 train | arrived choo railway
🚇 metro | subway travel
🚈 light rail | arrived monorail railway
🚉 station | railway train
🚊 tram | trolleybus
🚝 monorail | vehicle
🚞 mountain railway | car trip
🚋 tram car | bus trolley trolleybus
🚌 bus | school vehicle
🚍 oncoming bus | cars
🚎 trolleybus | bus tram trolley
🚐 minibus | bus drive van vehicle
🚑 ambulance | emergency vehicle
🚒 fire engine | truck
🚓 police car | 5–0 cops patrol
🚔 oncoming police car
🚕 taxi | cab cabbie car drive vehicle yellow
🚖 oncoming taxi | cab cabbie cars drove hail yellow
🚗 automobile | car driving vehicle
🚘 oncoming automobile | car cars drove vehicle
🚙 sport utility vehicle | car drive recreational sportutility
🛻 pickup truck | automobile car flatbed pick-up transportation
🚚 delivery truck | car drive vehicle
🚛 articulated lorry | car drive move semi truck vehicle
🚜 tractor | vehicle
🏎 racing car | zoom
🏍 motorcycle | racing
🛵 motor scooter
🦽 manual wheelchair | accessibility
🦼 motorized wheelchair | accessibility
🛺 auto rickshaw | tuk
🚲 bicycle | bike class cycle cycling cyclist gang ride spin spinning
🛴 kick scooter
🛹 skateboard | board skate skater wheels
🛼 roller skate | blades skates sport
🚏 bus stop | busstop
🛣 motorway | highway road
🛤 railway track | train
🛢 oil drum
⛽ fuel pump | diesel fuelpump gas gasoline station
🛞 wheel | car circle tire turn vehicle
🚨 police car light | alarm alert beacon emergency revolving siren
🚥 horizontal traffic light | intersection signal stop stoplight
🚦 vertical traffic light | drove intersection signal stop stoplight
🛑 stop sign | octagonal
🚧 construction | barrier
🛙 lighthouse | beacon enlightenment guidance hope light maritime navigation sea signal
⚓ anchor | ship tool
🛟 ring buoy | float life lifesaver preserver rescue safety save saver swim
⛵ sailboat | boat resort sailing sea yacht
🛶 canoe | boat
🚤 speedboat | billionaire boat lake luxury millionaire summer travel
🛳 passenger ship
⛴ ferry | boat passenger
🛥 motor boat | motorboat
🚢 ship | boat passenger travel
✈ airplane | aeroplane fly flying jet plane travel
🛩 small airplane | aeroplane plane
🛫 airplane departure | aeroplane check-in departures plane
🛬 airplane arrival | aeroplane arrivals arriving landing plane
🪂 parachute | hang-glide parasail skydive
💺 seat | chair
🚁 helicopter | copter roflcopter travel vehicle
🚟 suspension railway
🚠 mountain cableway | cable gondola lift ski
🚡 aerial tramway | cable car gondola ropeway
🛰 satellite | space
🚀 rocket | launch rockets space travel
🛸 flying saucer | aliens extra terrestrial ufo
🛎 bellhop bell | hotel
🧳 luggage | bag packing roller suitcase travel
⌛ hourglass done | sand time timer
⏳ hourglass not done | flowing hours sand timer waiting yolo
⌚ watch | clock time
⏰ alarm clock | hours hrs late time waiting
⏱ stopwatch | clock time
⏲ timer clock
🕰 mantelpiece clock | time mantel
🕛 twelve o'clock | 12 12:00 clock time
🕧 twelve-thirty | 12 12:30 30 clock thirty time twelve
🕐 one o'clock | 1 1:00 clock time
🕜 one-thirty | 1 1:30 30 clock one thirty time
🕑 two o'clock | 2 2:00 clock time
🕝 two-thirty | 2 2:30 30 clock thirty time two
🕒 three o'clock | 3 3:00 clock time
🕞 three-thirty | 3 3:30 30 clock thirty three time
🕓 four o'clock | 4 4:00 clock time
🕟 four-thirty | 30 4 4:30 clock four thirty time
🕔 five o'clock | 5 5:00 clock time
🕠 five-thirty | 30 5 5:30 clock five thirty time
🕕 six o'clock | 6 6:00 clock time
🕡 six-thirty | 30 6 6:30 clock six thirty
🕖 seven o'clock | 0 7 7:00 clock
🕢 seven-thirty | 30 7 7:30 clock seven thirty
🕗 eight o'clock | 8 8:00 clock time
🕣 eight-thirty | 30 8 8:30 clock eight thirty time
🕘 nine o'clock | 9 9:00 clock time
🕤 nine-thirty | 30 9 9:30 clock nine thirty time
🕙 ten o'clock | 0 10 10:00 clock
🕥 ten-thirty | 10 10:30 30 clock ten thirty time
🕚 eleven o'clock | 11 11:00 clock time
🕦 eleven-thirty | 11 11:30 30 clock eleven thirty time
";

const ACTIVITIES: &str = "
🎃 jack-o-lantern | celebration halloween jack lantern pumpkin
🎄 christmas tree | celebration
🎆 fireworks | boom celebration entertainment yolo
🎇 sparkler | boom celebration fireworks sparkle
🧨 firecracker | dynamite explosive fire fireworks light pop popping spark
✨ sparkles | * magic sparkle star clean
🎈 balloon | birthday celebrate celebration
🎉 party popper | awesome birthday celebrate celebration excited hooray tada woohoo
🎊 confetti ball | celebrate celebration party woohoo
🎋 tanabata tree | banner celebration japanese
🎍 pine decoration | bamboo celebration japanese plant
🎎 japanese dolls | celebration doll festival
🎏 carp streamer | celebration
🎐 wind chime | bell celebration
🎑 moon viewing ceremony | celebration
🧧 red envelope | gift good hóngbāo lai luck money see
🎀 ribbon | celebration
🎁 wrapped gift | birthday bow box celebration christmas present surprise
🎗 reminder ribbon | celebration
🎟 admission tickets | ticket
🎫 ticket | admission stub
🎖 military medal | award celebration
🏆 trophy | champion champs prize slay sport victory win winning
🏅 sports medal | award gold winner
🥇 1st place medal | first gold
🥈 2nd place medal | second silver
🥉 3rd place medal | bronze third
⚽ soccer ball | football futbol sport
⚾ baseball | ball sport
🥎 softball | ball glove sports underarm
🏀 basketball | ball hoop sport
🏐 volleyball | ball game
🏈 american football | ball bowl sport super
🏉 rugby football | ball sport
🎾 tennis | ball racquet sport
🥏 flying disc | ultimate
🎳 bowling | ball game sport strike
🏏 cricket game | ball bat
🏑 field hockey | ball game stick
🏒 ice hockey | game puck stick
🥍 lacrosse | ball goal sports stick
🏓 ping pong | ball bat game paddle pingpong table tennis
🏸 badminton | birdie game racquet shuttlecock
🥊 boxing glove
🥋 martial arts uniform | judo karate taekwondo
🥅 goal net
⛳ flag in hole | golf sport
⛸ ice skate | skating
🎣 fishing pole | entertainment fish sport
🤿 diving mask | scuba snorkeling
🎽 running shirt | athletics sash
🎿 skis | ski snow sport
🛷 sled | luge sledge sleigh snow toboggan
🥌 curling stone | game rock
🎯 bullseye | bull dart direct entertainment game hit target
🪀 yo-yo | fluctuate toy
🪁 kite | fly soar
🔫 water pistol | gun handgun revolver tool weapon
🎱 pool 8 ball | 8ball billiard eight game
🔮 crystal ball | fairy fairytale fantasy fortune future magic tale tool
🪄 magic wand | magician witch wizard
🎮 video game | controller entertainment
🕹 joystick | game video videogame
🎰 slot machine | casino gamble gambling game slots
🎲 game die | dice entertainment
🧩 puzzle piece | clue interlocking jigsaw
🧸 teddy bear | plaything plush stuffed toy
🪅 piñata | candy celebrate celebration cinco de festive mayo party pinada pinata
🪩 mirror ball | dance disco glitter party
🪆 nesting dolls | babooshka baboushka babushka doll matryoshka russia
♠ spade suit | card game
♥ heart suit | card emotion game hearts
♦ diamond suit | card game
♣ club suit | card clubs game
♟ chess pawn | dupe expendable
🃏 joker | card game wildcard
🀄 mahjong red dragon | game
🎴 flower playing cards | card game japanese
🎭 performing arts | actor actress art entertainment mask theater theatre thespian
🖼 framed picture | art frame museum painting
🎨 artist palette | art artsy arty colorful creative entertainment museum painter painting
🧵 thread | needle sewing spool string
🪡 sewing needle | embroidery sew stitches sutures tailoring thread
🧶 yarn | ball crochet knit wool knitting
🪢 knot | cord rope tangled tie twine twist
🔇 muted speaker | mute quiet silent sound
🔈 speaker low volume | soft sound
🔉 speaker medium volume | sound
🔊 speaker high volume | loud music sound
📢 loudspeaker | address communication loud public sound
📣 megaphone | cheering sound
📯 postal horn | post
🔔 bell | break church sound
🔕 bell with slash | forbidden mute no not prohibited quiet silent sound muted
🎼 musical score | music note
🎵 musical note | music sound
🎶 musical notes | music note sound
🎙 studio microphone | mic music
🎚 level slider | music
🎛 control knobs | music
🎤 microphone | karaoke mic music sing sound
🎧 headphone | earbud sound headphones
📻 radio | entertainment tbt video
🎷 saxophone | instrument music sax
🎺 trumpet | instrument music
🪊 trombone | brass instrument jazz music sad slide
🪗 accordion | box concertina instrument music squeeze squeezebox
🎸 guitar | instrument music strat
🎹 musical keyboard | instrument music piano
🎻 violin | instrument music
🪕 banjo | music stringed
🥁 drum | drumsticks music
🪘 long drum | beat conga instrument rhythm
🪇 maracas | cha dance instrument music party percussion rattle shake shaker
🪈 flute | band fife flautist instrument marching music orchestra piccolo pipe recorder woodwind
🪉 harp | cupid instrument love music orchestra
";

const SIGNS: &str = "
🏧 atm sign | automated bank cash money teller
🚮 litter in bin sign | litterbin
🚰 potable water | drinking
♿ wheelchair symbol | access handicap accessible
🚹 men's room | bathroom lavatory man restroom toilet wc men
🚺 women's room | bathroom lavatory restroom toilet wc woman women
🚻 restroom | bathroom lavatory toilet wc toilets
🚼 baby symbol | changing
🚾 water closet | bathroom lavatory restroom toilet wc
🛂 passport control
🛃 customs | packing
🛄 baggage claim | arrived bags case checked journey packing plane ready travel trip
🛅 left luggage | baggage case locker
⚠ warning | caution
🚸 children crossing | child pedestrian traffic
⛔ no entry | do fail forbidden not pass prohibited traffic
🚫 prohibited | entry forbidden no not smoke
🚳 no bicycles | bicycle bike forbidden not prohibited
🚭 no smoking | forbidden not prohibited smoke
🚯 no littering | forbidden litter not prohibited
🚱 non-potable water | dry non-drinking prohibited
🚷 no pedestrians | forbidden not pedestrian prohibited
📵 no mobile phones | cell forbidden not phone prohibited telephone
🔞 no one under eighteen | 18 age forbidden not prohibited restriction underage
☢ radioactive | sign
☣ biohazard | sign
⬆ up arrow | cardinal direction north
↗ up-right arrow | direction intercardinal northeast
➡ right arrow | cardinal direction east
↘ down-right arrow | direction intercardinal southeast
⬇ down arrow | cardinal direction south
↙ down-left arrow | direction intercardinal southwest
⬅ left arrow | cardinal direction west
↖ up-left arrow | direction intercardinal northwest
↕ up-down arrow | up down
↔ left-right arrow | left right
↩ right arrow curving left
↪ left arrow curving right
⤴ right arrow curving up
⤵ right arrow curving down
🔃 clockwise vertical arrows | arrow refresh reload
🔄 counterclockwise arrows button | again anticlockwise arrow deja refresh rewindershins vu
🔙 back arrow
🔚 end arrow
🔛 on! arrow | mark on
🔜 soon arrow | brb omw
🔝 top arrow | homie up
🛐 place of worship | pray religion
⚛ atom symbol | atheist
🕉 om | hindu religion
✡ star of david | jew jewish judaism religion
☸ wheel of dharma | buddhist religion
☯ yin yang | difficult lives religion tao taoist total yinyang
✝ latin cross | christ christian religion
☦ orthodox cross | christian religion
☪ star and crescent | islam muslim ramadan religion
☮ peace symbol | healing peaceful
🕎 menorah | candelabrum candlestick hanukkah jewish judaism religion
🔯 dotted six-pointed star | fortune jewish judaism
🪯 khanda | deg fateh khalsa religion sikh sikhism tegh
♈ aries | horoscope ram zodiac
♉ taurus | bull horoscope ox zodiac
♊ gemini | horoscope twins zodiac
♋ cancer | crab horoscope zodiac
♌ leo | horoscope lion zodiac
♍ virgo | horoscope zodiac
♎ libra | balance horoscope justice scales zodiac
♏ scorpio | horoscope scorpion scorpius zodiac
♐ sagittarius | archer horoscope zodiac
♑ capricorn | goat horoscope zodiac
♒ aquarius | bearer horoscope water zodiac
♓ pisces | fish horoscope zodiac
⛎ ophiuchus | bearer serpent snake zodiac
🔀 shuffle tracks button | arrow crossed
🔁 repeat button | arrow clockwise
🔂 repeat single button | arrow clockwise once
▶ play button | arrow right triangle
⏩ fast-forward button | arrow double fast forward
⏭ next track button | arrow scene triangle
⏯ play or pause button | arrow right triangle
◀ reverse button | arrow left triangle
⏪ fast reverse button | arrow double rewind
⏮ last track button | arrow previous scene triangle
🔼 upwards button | arrow red up
⏫ fast up button | arrow double
🔽 downwards button | arrow down red
⏬ fast down button | arrow double
⏸ pause button | bar double vertical
⏹ stop button | square
⏺ record button | circle
⏏ eject button
🎦 cinema | camera film movie
🔅 dim button | brightness low
🔆 bright button | brightness light
📶 antenna bars | bar cell communication mobile phone signal telephone
🛜 wireless | broadband computer connectivity hotspot internet network router smartphone wi-fi wifi wlan
📳 vibration mode | cell communication mobile phone telephone
📴 mobile phone off | cell telephone
♀ female sign | woman
♂ male sign | man
⚧ transgender symbol
✖ multiply | × cancel multiplication sign x
➕ plus | +
➖ minus | - − heavy math sign
➗ divide | ÷ division heavy math sign
🟰 heavy equals sign | answer equal equality math
♾ infinity | forever unbounded universal
‼ double exclamation mark | ! !! bangbang punctuation
⁉ exclamation question mark | ! !? ? interrobang punctuation
❓ red question mark | ? punctuation
❔ white question mark | ? outlined punctuation
❕ white exclamation mark | ! bang outlined punctuation
❗ red exclamation mark | ! bang punctuation
〰 wavy dash | punctuation
💱 currency exchange | bank money
💲 heavy dollar sign | billion cash charge currency million money pay
⚕ medical symbol | aesculapius medicine staff
♻ recycling symbol | recycle
⚜ fleur-de-lis | knights
🔱 trident emblem | anchor poseidon ship tool
📛 name badge
🔰 japanese symbol for beginner | chevron green leaf tool yellow
⭕ hollow red circle | heavy large o
✅ check mark button | ✓ checked checkmark complete completed done fixed tick
☑ check box with check | ✓ ballot checked done off tick
✔ check mark | ✓ checked checkmark done heavy tick
❌ cross mark | × cancel multiplication multiply x
❎ cross mark button | × multiplication multiply square x
➰ curly loop | curl
➿ double curly loop | curl
〽 part alternation mark
✳ eight-spoked asterisk | *
✴ eight-pointed star | *
❇ sparkle | *
© copyright | c
® registered | r
™ trade mark | tm trademark
🫟 splatter | drip holi ink liquid mess paint spill stain
🔟 keycap: 10
🔠 input latin uppercase | abcd letters
🔡 input latin lowercase | abcd letters
🔢 input numbers | 1234
🔣 input symbols | & % ♪ 〒
🔤 input latin letters | abc alphabet
🅰 a button (blood type) | blood type
🆎 ab button (blood type) | blood type
🅱 b button (blood type) | blood type
🆑 cl button
🆒 cool button
🆓 free button
ℹ information | i
🆔 id button | identity
Ⓜ circled m | circle
🆕 new button
🆖 ng button
🅾 o button (blood type) | blood type
🆗 ok button | okay
🅿 p button | parking
🆘 sos button | help
🆙 up! button | mark up
🆚 vs button | versus
🈁 japanese “here” button | here katakana
🈂 japanese “service charge” button | charge katakana service
🈷 japanese “monthly amount” button | amount ideograph monthly
🈶 japanese “not free of charge” button | charge ideograph not
🈯 japanese “reserved” button | ideograph reserved
🉐 japanese “bargain” button | bargain ideograph
🈹 japanese “discount” button | discount ideograph
🈚 japanese “free of charge” button | charge free ideograph
🈲 japanese “prohibited” button | ideograph prohibited
🉑 japanese “acceptable” button | acceptable ideograph
🈸 japanese “application” button | application ideograph
🈴 japanese “passing grade” button | grade ideograph passing
🈳 japanese “vacancy” button | ideograph vacancy
㊗ japanese “congratulations” button | congratulations ideograph
㊙ japanese “secret” button | ideograph secret
🈺 japanese “open for business” button | business ideograph open
🈵 japanese “no vacancy” button | ideograph no vacancy
🔴 red circle | geometric
🟠 orange circle
🟡 yellow circle
🟢 green circle
🔵 blue circle | geometric
🟣 purple circle
🟤 brown circle
⚫ black circle | geometric
⚪ white circle | geometric
🟥 red square | card penalty
🟧 orange square
🟨 yellow square | card penalty
🟩 green square
🟦 blue square
🟪 purple square
🟫 brown square
⬛ black large square | geometric
⬜ white large square | geometric
◼ black medium square | geometric
◻ white medium square | geometric
◾ black medium-small square | geometric
◽ white medium-small square | geometric
▪ black small square | geometric
▫ white small square | geometric
🔶 large orange diamond | geometric
🔷 large blue diamond | geometric
🔸 small orange diamond | geometric
🔹 small blue diamond | geometric
🔺 red triangle pointed up | geometric
🔻 red triangle pointed down | geometric
💠 diamond with a dot | comic geometric
🔘 radio button | geometric
🔳 white square button | geometric outlined
🔲 black square button | geometric
🏁 chequered flag | checkered finish flags game race racing sport win
🚩 triangular flag | construction golf post
🎌 crossed flags | celebration cross japanese
🏴 black flag | waving
🏳 white flag | waving
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
                !emoji.name.is_empty()
                    && emoji.name == emoji.name.to_lowercase()
                    && emoji.words == emoji.words.to_lowercase(),
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
        assert!(seen.len() > 1200, "{} emoji", seen.len());
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
        // The name is what comes before the bar, and the words to find it by what comes after.
        let apple = parse("🍎 red apple | fruit healthy").unwrap();
        assert_eq!((apple.name, apple.words), ("red apple", "fruit healthy"));
        let plain = parse("🍎 apple").unwrap();
        assert_eq!((plain.name, plain.words), ("apple", ""));
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
        // The start of a word will do, which is how a search finds as you type, but the middle of
        // one will not: the pineapple is not an apple, and people who grapple do not have one.
        assert!(found("carr").contains(&'🥕'));
        assert!(found("pine").contains(&'🍍'));
        assert!(!found("apple").contains(&'🍍'));
        assert!(!found("apple").contains(&'🤼'));
        assert!(found("app").contains(&'🍎'));
        assert!(found("   ").is_empty());
        assert!(found("").is_empty());
        assert!(found("zzzzzz").is_empty());
    }

    #[test]
    fn the_clown_and_other_things_that_were_missed_are_there() {
        let found = |query: &str| -> Vec<char> {
            search(query)
                .into_iter()
                .map(|emoji| emoji.symbol)
                .collect()
        };
        assert!(found("clown").contains(&'🤡'));
        assert!(found("octopus").contains(&'🐙'));
        assert!(found("pizza").contains(&'🍕'));
        assert!(found("rocket").contains(&'🚀'));
        // Words of the keywords find them too, not only their names: a house is also a home.
        assert!(found("home").contains(&'🏠'));
        assert!(found("heart").contains(&'❤'));
    }

    #[test]
    fn what_the_built_in_font_can_draw_is_most_of_the_list_and_can_be_told_apart() {
        // Without the installed fonts only the built-in emoji font draws them, and it is old.
        let fonts = Fonts::embedded();
        let drawn = all()
            .into_iter()
            .filter(|emoji| fonts.fallback_draws(emoji.symbol))
            .count();
        assert!(drawn > 500, "only {drawn} of the emoji can be drawn");
        assert!(fonts.fallback_draws('🍎'));
        assert!(!fonts.fallback_draws('x'));
    }

    #[test]
    fn with_the_fonts_of_this_computer_most_of_the_list_can_be_drawn() {
        let fonts = Fonts::system();
        // Only where a recent emoji font is installed, as Windows has.
        if fonts.installed_emoji_font().is_none() {
            return;
        }
        // The newest emoji are not in every Windows, and the window does not offer what it cannot
        // draw, so a few of them missing is as it should be.
        let all = all();
        let missing: Vec<char> = all
            .iter()
            .map(|emoji| emoji.symbol)
            .filter(|&symbol| !fonts.fallback_draws(symbol))
            .collect();
        assert!(
            missing.len() * 5 < all.len(),
            "{} of {} emoji cannot be drawn: {missing:?}",
            missing.len(),
            all.len()
        );
    }
}
