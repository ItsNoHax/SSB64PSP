//! Golden-capture scene specs.
//!
//! A `golden_capture` EBOOT reads one line from `capture_scene.txt` beside
//! it and renders that scene, so one build covers every golden instead of one
//! Cargo feature (and one build) per scene. The same one-line spec is the
//! `scene_spec` column of `tests/golden/scenes.tsv`.
//!
//! This crate is `no_std`, allocation-free and free of PSP types so that the
//! parser runs in host tests. The scene behaviour itself (which object, which
//! stage, which light) stays in each PSP binary.

#![no_std]

use core::fmt;

/// Longest spec either binary accepts. `capture_scene.txt` is read into a
/// fixed buffer of this size.
pub const MAX_SPEC_LEN: usize = 64;

/// The file name both binaries look for next to their EBOOT.
pub const SCENE_FILE: &str = "capture_scene.txt";

/// The first non-empty, non-comment line of a scene file, trimmed.
pub fn spec_line(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
}

/// A fighter with a neutral-model golden in `psp-asset-viewer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fighter {
    Mario,
    Fox,
    DonkeyKong,
    Samus,
    Luigi,
    Link,
    Yoshi,
    CaptainFalcon,
    Kirby,
    Pikachu,
    Purin,
    Ness,
    MetalMario,
}

impl Fighter {
    pub const ALL: [Fighter; 13] = [
        Fighter::Mario,
        Fighter::Fox,
        Fighter::DonkeyKong,
        Fighter::Samus,
        Fighter::Luigi,
        Fighter::Link,
        Fighter::Yoshi,
        Fighter::CaptainFalcon,
        Fighter::Kirby,
        Fighter::Pikachu,
        Fighter::Purin,
        Fighter::Ness,
        Fighter::MetalMario,
    ];

    /// The spec name, which is also the old `regression_capture_<name>`
    /// feature suffix.
    pub const fn name(self) -> &'static str {
        match self {
            Fighter::Mario => "mario",
            Fighter::Fox => "fox",
            Fighter::DonkeyKong => "donkey_kong",
            Fighter::Samus => "samus",
            Fighter::Luigi => "luigi",
            Fighter::Link => "link",
            Fighter::Yoshi => "yoshi",
            Fighter::CaptainFalcon => "captain_falcon",
            Fighter::Kirby => "kirby",
            Fighter::Pikachu => "pikachu",
            Fighter::Purin => "purin",
            Fighter::Ness => "ness",
            Fighter::MetalMario => "metal_mario",
        }
    }

    pub fn from_name(name: &str) -> Option<Fighter> {
        Fighter::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// One `psp-asset-viewer` golden scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerScene {
    /// `default`: Dream Land stage view, Mario at spawn 0.
    DreamLand,
    /// `scene2`: file 52 opening-movie graph.
    OpeningRoom,
    /// `scene3`: file 109 graph 0x44C8.
    StageSector,
    /// `scene4`: file 84 graph 0x2760.
    CatchSwirl,
    /// `scene5`: stage 9, Saffron City gate.
    SaffronGate,
    /// `scene6`: file 117 graph 0x1B10.
    MetalTexgen,
    /// `scene7`: scene 6 a quarter turn round.
    MetalTexgenRotated,
    /// `scene8`: file 117 graph 0x2EE0.
    MetalTexgenLinear,
    /// `scene9`: scene 6 under a rotated camera.
    MetalTexgenCameraRotated,
    /// `scene10`: stage 4, Peach's Castle.
    PeachCastle,
    /// `stage N`: whole-stage view of stage index N.
    Stage(u32),
    /// `fighter NAME`: neutral high-detail model.
    Fighter(Fighter),
    /// `link_costume_1`: Link with costume 1.
    LinkCostume1,
    /// `skeleton NAME`: the fighter's first electric-damage skeleton set
    /// (RE-414) in place of its model.
    Skeleton(Fighter),
    /// `mario_entry`: file 356 graph 0x608 in the effect browser.
    MarioEntry,
    /// `bonus_platform`: file 136 graph 0x3DA8.
    BonusPlatform,
    /// `depth_mask`: synthetic depth-write ON/OFF/ON quads.
    DepthMask,
    /// `dream_land_water`: file 104 graph 0x2450, Dream Land's water layer,
    /// seen from above: both two-tile fractional blend ponds (RE-321).
    DreamLandWater,
}

impl ViewerScene {
    /// Parses one spec, e.g. `stage 17`, `fighter fox`, `scene3`.
    pub fn parse(spec: &str) -> Option<ViewerScene> {
        let mut words = spec.split_whitespace();
        let head = words.next()?;
        let arg = words.next();
        if words.next().is_some() {
            return None;
        }
        let scene = match (head, arg) {
            ("default", None) => ViewerScene::DreamLand,
            ("scene2", None) => ViewerScene::OpeningRoom,
            ("scene3", None) => ViewerScene::StageSector,
            ("scene4", None) => ViewerScene::CatchSwirl,
            ("scene5", None) => ViewerScene::SaffronGate,
            ("scene6", None) => ViewerScene::MetalTexgen,
            ("scene7", None) => ViewerScene::MetalTexgenRotated,
            ("scene8", None) => ViewerScene::MetalTexgenLinear,
            ("scene9", None) => ViewerScene::MetalTexgenCameraRotated,
            ("scene10", None) => ViewerScene::PeachCastle,
            ("stage", Some(n)) => ViewerScene::Stage(parse_decimal(n)?),
            ("fighter", Some(name)) => ViewerScene::Fighter(Fighter::from_name(name)?),
            ("link_costume_1", None) => ViewerScene::LinkCostume1,
            ("skeleton", Some(name)) => ViewerScene::Skeleton(Fighter::from_name(name)?),
            ("mario_entry", None) => ViewerScene::MarioEntry,
            ("bonus_platform", None) => ViewerScene::BonusPlatform,
            ("depth_mask", None) => ViewerScene::DepthMask,
            ("dream_land_water", None) => ViewerScene::DreamLandWater,
            _ => return None,
        };
        Some(scene)
    }

    /// The fighter whose model, `Wait` animation and light this scene shows.
    pub const fn fighter(self) -> Option<Fighter> {
        match self {
            ViewerScene::Fighter(f) | ViewerScene::Skeleton(f) => Some(f),
            ViewerScene::LinkCostume1 => Some(Fighter::Link),
            _ => None,
        }
    }

    /// The stage shown by a whole-stage scene. `None` keeps the viewer's
    /// default stage 0.
    pub const fn stage_index(self) -> Option<u32> {
        match self {
            ViewerScene::SaffronGate => Some(9),
            ViewerScene::PeachCastle => Some(4),
            ViewerScene::Stage(n) => Some(n),
            _ => None,
        }
    }

    /// The `(source file, graph offset)` of the object a scene selects by
    /// exact graph. Fighters come from their own table in the viewer.
    pub const fn object_graph(self) -> Option<(u32, u32)> {
        match self {
            ViewerScene::StageSector => Some((109, 0x44C8)),
            ViewerScene::CatchSwirl => Some((84, 0x2760)),
            ViewerScene::MetalTexgen
            | ViewerScene::MetalTexgenRotated
            | ViewerScene::MetalTexgenCameraRotated => Some((117, 0x1B10)),
            ViewerScene::MetalTexgenLinear => Some((117, 0x2EE0)),
            ViewerScene::BonusPlatform => Some((136, 0x3DA8)),
            ViewerScene::DreamLandWater => Some((104, 0x2450)),
            _ => None,
        }
    }

    /// `(yaw, pitch, distance scale)`, angles in radians, of a real camera
    /// orbiting the object's centre, for scenes that need a view the object
    /// viewer's spin cannot give. `None` keeps the identity view.
    pub const fn orbit_camera(self) -> Option<(f32, f32, f32)> {
        match self {
            // 35 and 20 degrees.
            ViewerScene::MetalTexgenCameraRotated => Some((0.610_865_2, 0.349_065_85, 1.0)),
            // 60 degrees down onto the ponds, facing the stage front, close
            // enough that each pond spans a few hundred pixels.
            ViewerScene::DreamLandWater => Some((0.0, core::f32::consts::FRAC_PI_3, 0.4)),
            _ => None,
        }
    }

    /// Scenes that start in the object viewer instead of the stage view.
    pub const fn object_view(self) -> bool {
        matches!(
            self,
            ViewerScene::OpeningRoom
                | ViewerScene::StageSector
                | ViewerScene::CatchSwirl
                | ViewerScene::MetalTexgen
                | ViewerScene::MetalTexgenRotated
                | ViewerScene::MetalTexgenLinear
                | ViewerScene::MetalTexgenCameraRotated
                | ViewerScene::Fighter(_)
                | ViewerScene::Skeleton(_)
                | ViewerScene::LinkCostume1
                | ViewerScene::MarioEntry
                | ViewerScene::BonusPlatform
                | ViewerScene::DreamLandWater
        )
    }

    /// Scenes whose object does not drift round. The bonus platform and the
    /// entry pipe predate this list and drift until the freeze; their goldens
    /// pin that angle.
    pub const fn holds_spin(self) -> bool {
        matches!(
            self,
            ViewerScene::OpeningRoom
                | ViewerScene::StageSector
                | ViewerScene::CatchSwirl
                | ViewerScene::MetalTexgen
                | ViewerScene::MetalTexgenRotated
                | ViewerScene::MetalTexgenLinear
                | ViewerScene::MetalTexgenCameraRotated
                | ViewerScene::Fighter(_)
                | ViewerScene::Skeleton(_)
                | ViewerScene::LinkCostume1
                | ViewerScene::DreamLandWater
        )
    }
}

impl fmt::Display for ViewerScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewerScene::DreamLand => f.write_str("default"),
            ViewerScene::OpeningRoom => f.write_str("scene2"),
            ViewerScene::StageSector => f.write_str("scene3"),
            ViewerScene::CatchSwirl => f.write_str("scene4"),
            ViewerScene::SaffronGate => f.write_str("scene5"),
            ViewerScene::MetalTexgen => f.write_str("scene6"),
            ViewerScene::MetalTexgenRotated => f.write_str("scene7"),
            ViewerScene::MetalTexgenLinear => f.write_str("scene8"),
            ViewerScene::MetalTexgenCameraRotated => f.write_str("scene9"),
            ViewerScene::PeachCastle => f.write_str("scene10"),
            ViewerScene::Stage(n) => write!(f, "stage {n}"),
            ViewerScene::Fighter(fighter) => write!(f, "fighter {}", fighter.name()),
            ViewerScene::LinkCostume1 => f.write_str("link_costume_1"),
            ViewerScene::Skeleton(fighter) => write!(f, "skeleton {}", fighter.name()),
            ViewerScene::MarioEntry => f.write_str("mario_entry"),
            ViewerScene::BonusPlatform => f.write_str("bonus_platform"),
            ViewerScene::DepthMask => f.write_str("depth_mask"),
            ViewerScene::DreamLandWater => f.write_str("dream_land_water"),
        }
    }
}

/// One `psp-game` scripted Training scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameScene {
    /// `training`: jump, jab, freeze.
    Training,
    /// `fireball`: Training plus Mario's neutral B.
    Fireball,
    /// `superjump`: Training plus Mario's up B.
    Superjump,
    /// `fox`: Fox in Training, one Blaster shot.
    Fox,
    /// `luigi`: Luigi in Training, one Fireball.
    Luigi,
    /// `samus`: Samus in Training, charging a Charge Shot.
    Samus,
    /// `samusshot`: Samus in Training, one released Charge Shot.
    SamusShot,
    /// `samusbomb`: Samus in Training, one Bomb on the floor.
    SamusBomb,
    /// `link`: Link in Training, one Boomerang in flight.
    Link,
    /// `linkspin`: Link in Training, mid ground Spin Attack.
    LinkSpin,
    /// `yoshi`: Yoshi in Training, one Egg Throw egg in flight.
    Yoshi,
    /// `yoshibomb`: Yoshi in Training, the Yoshi Bomb's two landing stars.
    YoshiBomb,
    /// `captain`: Captain Falcon in Training, mid Falcon Punch with its flame.
    Captain,
    /// `captainkick`: Captain Falcon in Training, mid ground Falcon Kick.
    CaptainKick,
    /// `kirby`: Kirby in Training, one Final Cutter wave on the floor.
    Kirby,
    /// `pikachu`: Pikachu in Training, one Thunder Jolt crawling the floor.
    Pikachu,
    /// `pikachuair`: Pikachu in Training, one aerial Thunder Jolt after a
    /// jump.
    PikachuAir,
    /// `purin`: Jigglypuff in Training, mid Sing with its notes.
    Purin,
    /// `donkey`: Donkey Kong in Training, charging a Giant Punch.
    Donkey,
    /// `ness`: Ness in Training, one PK Fire spark in flight.
    Ness,
    /// `nessthunder`: Ness in Training, PK Thunder's head and trails.
    NessThunder,
    /// `nessmagnet`: Ness in Training, holding PSI Magnet.
    NessMagnet,
    /// `linkbomb`: Link in Training, a Bomb just pulled into his hand.
    LinkBomb,
    /// `shadows`: first jump's apex, player airborne.
    Shadows,
    /// `grab`: Training plus a Z+A grab of the dummy, frozen while held.
    Grab,
    /// `jab`: the grab scene's route onto the dummy's platform, then a jab
    /// that lands, frozen in the dummy's damage status.
    Jab,
    /// `shield`: Mario holding a diagonally tilted shield in Training.
    Shield,
    /// `costume1`..`costume3`: Fox in Training after a C-Right, C-Down or
    /// C-Left costume pick on the Training menu entry.
    Costume1,
    Costume2,
    Costume3,
    /// `stageselect`: Mario in Training on Hyrule Castle, picked on the
    /// stage select. Every other scene skips the select and loads Dream
    /// Land.
    StageSelect,
    /// `fighterselect`: Kirby picked on the character select, then Peach's
    /// Castle on the stage select, in Training.
    FighterSelect,
    /// `rebirth`: Mario dashes off Dream Land, is KO'd below the stage and
    /// comes back under the rebirth halo.
    Rebirth,
    /// `vs`: a VS battle's countdown, Mario against a Mario that stands
    /// still.
    Vs,
    /// `vstimeup`: a one-minute VS battle run to its results.
    VsTimeUp,
    /// `cpuwalk`: the Training dummy under the CPU's Walk behaviour.
    CpuWalk,
    /// `cpujump`: the Training dummy under the CPU's Jump behaviour.
    CpuJump,
    /// `vscpu`: a VS battle's CPU closing in and attacking.
    VsCpu,
    /// `vstimeupsign`: `vstimeup`'s "TIME UP" before sudden death.
    VsTimeUpSign,
    /// `vssuddendeath`: `vstimeup`'s "SUDDEN DEATH!" before its "GO!".
    VsSuddenDeath,
    /// `vspause`: a VS battle paused, the camera zoomed on the player.
    VsPause,
    /// `vsmode`: the VS mode menu set to a four-stock battle.
    VsModeMenu,
    /// `vsnocontest`: a VS battle reset from the pause menu, at its
    /// no-contest results.
    VsNoContest,
    /// `vsplayers`: the VS character select with the player's fighter
    /// placed and a CPU opened.
    VsPlayers,
    /// `vs4`: a VS battle after "Go", Mario against three CPUs.
    Vs4,
    /// `vsteam`: a VS team battle after "Go", Mario and a CPU on red
    /// against two CPUs on blue.
    VsTeam,
    /// `vsresults`: a one-stock VS battle the player loses by running off
    /// the stage, at its results: the winner in its Win pose, the player
    /// clapping.
    VsResults,
    /// `rebirthblast`: `rebirth` caught a few ticks after the KO, the
    /// blast explosion and the screen flash up (RE-412).
    RebirthBlast,
    /// `pikachuthunder`: Pikachu in Training, Thunder's trails and fading
    /// segments down to him (RE-417).
    PikachuThunder,
    /// `kirbyhat`: Kirby in Training after inhaling and copying the Mario
    /// dummy, wearing Mario's cap (RE-417).
    KirbyHat,
    /// `yoshiegg`: Yoshi in Training after an Egg Lay of the Mario dummy,
    /// the dummy in its egg (RE-417).
    YoshiEgg,
    /// `yoshishield`: Yoshi in Training holding his egg shield until its
    /// health has worn and the egg has darkened (RE-418).
    YoshiShield,
    /// `yoshirollf` / `yoshirollb`: guard then roll in either direction.
    YoshiRollF,
    YoshiRollB,
    /// `yoshishieldbreak`: release the egg shield and draw its fragments.
    YoshiShieldBreak,
    /// `vsshield`: a VS battle whose player holds his shield from "Go",
    /// caught on the frame a CPU's hit sets it off, in the grey damage
    /// colour (RE-418).
    VsShield,
    /// `stageselectview`: the Training stage select on Hyrule Castle, its
    /// preview model turning over the stage's Training wallpaper (RE-419).
    StageSelectView,
    /// `stageselectyoshi`: the Training stage select on Yoshi's Island,
    /// the preview hiding the two nodes `mnMapsMakeModel` hides (RE-419).
    StageSelectYoshi,
    /// `vssector`: `vs`'s countdown on Sector Z, whose wallpaper scales
    /// with the camera's distance (RE-419).
    VsSector,
    /// `vsyoshi`: `vs`'s countdown on Yoshi's Island, whose wallpaper
    /// stands still (RE-419).
    VsYoshi,
    /// `trainingselect`: the Training character select on its first visit,
    /// the portraits in, the CPU's fighter turning and the hand on the
    /// player's card.
    TrainingSelect,
    /// `trainingselectpicked`: the Training select after Kirby is placed in
    /// his C-Down costume and the CPU's puck is picked up and placed again
    /// on Mario with C-Right, the ready banner up.
    TrainingSelectPicked,
    /// `starko`: `rebirth`'s Mario put in `DeadUpStar` on the floor, caught
    /// flying away past 10,000 units from the camera (RE-420).
    StarKo,
    /// `vsresultsemblem`: `vsresults` at an earlier results tic, the
    /// winner's series emblem shrinking and rising in the winner's colour
    /// (RE-420).
    VsResultsEmblem,
    /// `trainingjungle`: Training on Kongo Jungle, both fighters settled at
    /// their spawns under the camera's rest framing: its stage layer 3 (the
    /// rope rail) over the fighters (RE-422).
    TrainingJungle,
    /// `trainingzebes`: Training on Planet Zebes; layer 1's head-1 lists
    /// and the acid (RE-422).
    TrainingZebes,
    /// `trainingsaffron`: Training on Saffron City; layer 3's front rail
    /// and building (RE-422).
    TrainingSaffron,
    /// `traininginishie`: Training on Mushroom Kingdom; layer 3's fences
    /// (RE-422).
    TrainingInishie,
    /// `trainingyoster`: Training on Yoshi's Island; the fruit panel's
    /// tile-loaded strips and the platforms (RE-423).
    TrainingYoster,
    /// `trainingsector`: Training on Sector Z (RE-423).
    TrainingSector,
    /// `trainingarwing`: `trainingsector` at tick 1800, the Arwing's wing
    /// passing over the stage (RE-428).
    TrainingArwing,
    /// `trainingcastle`: Training on Peach's Castle (RE-423).
    TrainingCastle,
    /// `trainingbumper`: Castle at tick 240, camera centered on its Bumper
    /// for item rendering checks (RE-429).
    TrainingBumper,
    /// `trainingplants`: Mushroom Kingdom at tick 240, camera centered on
    /// the left pipe's Piranha Plant (RE-429).
    TrainingPlants,
    /// Diagnostic loose Capsule view (RE-431).
    TrainingCapsule,
    TrainingCrate,
    TrainingBarrel,
    TrainingHeavy,
    /// `trainingutility`: a Tomato within Mario's reach, a Heart and a
    /// Star; A at tick 60 picks up and eats the Tomato (RE-433).
    TrainingUtility,
    /// Diagnostic Saffron item views: source lifecycle, selected maker.
    TrainingChansey,
    TrainingElectrode,
    TrainingCharmander,
    TrainingVenusaur,
    TrainingPorygon,

    /// `traininghyrule`: Training on Hyrule Castle (RE-423).
    TrainingHyrule,
    /// `trainingpupupu`: Training on Dream Land at the stage scenes' tick
    /// (RE-423).
    TrainingPupupu,
    /// `pikachuhat`: Training's Pikachu in costume 1 against a Jigglypuff
    /// dummy in costume 2, both wearing their headgear accessory (RE-425).
    PikachuHat,
    /// `purinbow`: Training's Jigglypuff in costume 3 against a Pikachu
    /// dummy in costume 3 (RE-425).
    PurinBow,
    /// `vsarwing`: a VS battle's entries, Fox against Captain Falcon, Fox's
    /// Arwing mid-flight (RE-425).
    VsArwing,
    /// `vscar`: `vsarwing` later, Captain Falcon's car driving in (RE-425).
    VsCar,
    /// `vsball`: a VS battle's entries, Pikachu against Jigglypuff, the
    /// Poké Ball in flight (RE-425).
    VsBall,
    /// `vsrays`: `vsball` later, the ball open under its rays (RE-425).
    VsRays,
}

impl GameScene {
    pub const ALL: [GameScene; 92] = [
        GameScene::Training,
        GameScene::Fireball,
        GameScene::Superjump,
        GameScene::Fox,
        GameScene::Luigi,
        GameScene::Samus,
        GameScene::SamusShot,
        GameScene::SamusBomb,
        GameScene::Link,
        GameScene::LinkSpin,
        GameScene::Yoshi,
        GameScene::YoshiBomb,
        GameScene::Captain,
        GameScene::CaptainKick,
        GameScene::Kirby,
        GameScene::Pikachu,
        GameScene::PikachuAir,
        GameScene::Purin,
        GameScene::Donkey,
        GameScene::Ness,
        GameScene::NessThunder,
        GameScene::NessMagnet,
        GameScene::LinkBomb,
        GameScene::Shadows,
        GameScene::Grab,
        GameScene::Jab,
        GameScene::Shield,
        GameScene::Costume1,
        GameScene::Costume2,
        GameScene::Costume3,
        GameScene::StageSelect,
        GameScene::FighterSelect,
        GameScene::Rebirth,
        GameScene::Vs,
        GameScene::VsTimeUp,
        GameScene::CpuWalk,
        GameScene::CpuJump,
        GameScene::VsCpu,
        GameScene::VsTimeUpSign,
        GameScene::VsSuddenDeath,
        GameScene::VsPause,
        GameScene::VsModeMenu,
        GameScene::VsNoContest,
        GameScene::VsPlayers,
        GameScene::Vs4,
        GameScene::VsTeam,
        GameScene::VsResults,
        GameScene::RebirthBlast,
        GameScene::PikachuThunder,
        GameScene::KirbyHat,
        GameScene::YoshiEgg,
        GameScene::YoshiShield,
        GameScene::YoshiRollF,
        GameScene::YoshiRollB,
        GameScene::YoshiShieldBreak,
        GameScene::VsShield,
        GameScene::StageSelectView,
        GameScene::StageSelectYoshi,
        GameScene::VsSector,
        GameScene::VsYoshi,
        GameScene::TrainingSelect,
        GameScene::TrainingSelectPicked,
        GameScene::StarKo,
        GameScene::VsResultsEmblem,
        GameScene::TrainingJungle,
        GameScene::TrainingZebes,
        GameScene::TrainingSaffron,
        GameScene::TrainingInishie,
        GameScene::TrainingYoster,
        GameScene::TrainingSector,
        GameScene::TrainingArwing,
        GameScene::TrainingCastle,
        GameScene::TrainingBumper,
        GameScene::TrainingPlants,
        GameScene::TrainingCapsule,
        GameScene::TrainingCrate,
        GameScene::TrainingBarrel,
        GameScene::TrainingHeavy,
        GameScene::TrainingUtility,
        GameScene::TrainingChansey,
        GameScene::TrainingElectrode,
        GameScene::TrainingCharmander,
        GameScene::TrainingVenusaur,
        GameScene::TrainingPorygon,
        GameScene::TrainingHyrule,
        GameScene::TrainingPupupu,
        GameScene::PikachuHat,
        GameScene::PurinBow,
        GameScene::VsArwing,
        GameScene::VsCar,
        GameScene::VsBall,
        GameScene::VsRays,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            GameScene::Training => "training",
            GameScene::Fireball => "fireball",
            GameScene::Superjump => "superjump",
            GameScene::Fox => "fox",
            GameScene::Luigi => "luigi",
            GameScene::Samus => "samus",
            GameScene::SamusShot => "samusshot",
            GameScene::SamusBomb => "samusbomb",
            GameScene::Link => "link",
            GameScene::LinkSpin => "linkspin",
            GameScene::Yoshi => "yoshi",
            GameScene::YoshiBomb => "yoshibomb",
            GameScene::Captain => "captain",
            GameScene::CaptainKick => "captainkick",
            GameScene::Kirby => "kirby",
            GameScene::Pikachu => "pikachu",
            GameScene::PikachuAir => "pikachuair",
            GameScene::Purin => "purin",
            GameScene::Donkey => "donkey",
            GameScene::Ness => "ness",
            GameScene::NessThunder => "nessthunder",
            GameScene::NessMagnet => "nessmagnet",
            GameScene::LinkBomb => "linkbomb",
            GameScene::Shadows => "shadows",
            GameScene::Grab => "grab",
            GameScene::Jab => "jab",
            GameScene::Shield => "shield",
            GameScene::Costume1 => "costume1",
            GameScene::Costume2 => "costume2",
            GameScene::Costume3 => "costume3",
            GameScene::StageSelect => "stageselect",
            GameScene::FighterSelect => "fighterselect",
            GameScene::Rebirth => "rebirth",
            GameScene::Vs => "vs",
            GameScene::VsTimeUp => "vstimeup",
            GameScene::CpuWalk => "cpuwalk",
            GameScene::CpuJump => "cpujump",
            GameScene::VsCpu => "vscpu",
            GameScene::VsTimeUpSign => "vstimeupsign",
            GameScene::VsSuddenDeath => "vssuddendeath",
            GameScene::VsPause => "vspause",
            GameScene::VsModeMenu => "vsmode",
            GameScene::VsNoContest => "vsnocontest",
            GameScene::VsPlayers => "vsplayers",
            GameScene::Vs4 => "vs4",
            GameScene::VsTeam => "vsteam",
            GameScene::VsResults => "vsresults",
            GameScene::RebirthBlast => "rebirthblast",
            GameScene::PikachuThunder => "pikachuthunder",
            GameScene::KirbyHat => "kirbyhat",
            GameScene::YoshiEgg => "yoshiegg",
            GameScene::YoshiShield => "yoshishield",
            GameScene::YoshiRollF => "yoshirollf",
            GameScene::YoshiRollB => "yoshirollb",
            GameScene::YoshiShieldBreak => "yoshishieldbreak",
            GameScene::VsShield => "vsshield",
            GameScene::StageSelectView => "stageselectview",
            GameScene::StageSelectYoshi => "stageselectyoshi",
            GameScene::VsSector => "vssector",
            GameScene::VsYoshi => "vsyoshi",
            GameScene::TrainingSelect => "trainingselect",
            GameScene::TrainingSelectPicked => "trainingselectpicked",
            GameScene::StarKo => "starko",
            GameScene::VsResultsEmblem => "vsresultsemblem",
            GameScene::TrainingJungle => "trainingjungle",
            GameScene::TrainingZebes => "trainingzebes",
            GameScene::TrainingSaffron => "trainingsaffron",
            GameScene::TrainingInishie => "traininginishie",
            GameScene::TrainingYoster => "trainingyoster",
            GameScene::TrainingSector => "trainingsector",
            GameScene::TrainingArwing => "trainingarwing",
            GameScene::TrainingCastle => "trainingcastle",
            GameScene::TrainingBumper => "trainingbumper",
            GameScene::TrainingPlants => "trainingplants",
            GameScene::TrainingCapsule => "trainingcapsule",
            GameScene::TrainingCrate => "trainingcrate",
            GameScene::TrainingBarrel => "trainingbarrel",
            GameScene::TrainingHeavy => "trainingheavy",
            GameScene::TrainingUtility => "trainingutility",
            GameScene::TrainingChansey => "trainingchansey",
            GameScene::TrainingElectrode => "trainingelectrode",
            GameScene::TrainingCharmander => "trainingcharmander",
            GameScene::TrainingVenusaur => "trainingvenusaur",
            GameScene::TrainingPorygon => "trainingporygon",

            GameScene::TrainingHyrule => "traininghyrule",
            GameScene::TrainingPupupu => "trainingpupupu",
            GameScene::PikachuHat => "pikachuhat",
            GameScene::PurinBow => "purinbow",
            GameScene::VsArwing => "vsarwing",
            GameScene::VsCar => "vscar",
            GameScene::VsBall => "vsball",
            GameScene::VsRays => "vsrays",
        }
    }

    pub fn parse(spec: &str) -> Option<GameScene> {
        let spec = spec.trim();
        GameScene::ALL.into_iter().find(|s| s.name() == spec)
    }
}

impl fmt::Display for GameScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A game capture scene and optional deterministic capture tick (RE-426).
/// The `scene@tick` format also serves N64 frame comparisons (RE-425).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSpec {
    pub scene: GameScene,
    pub tick: Option<u32>,
}

impl GameSpec {
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        let (name, tick) = match spec.split_once('@') {
            Some((name, tick)) => {
                let tick = parse_decimal(tick)?;
                if tick == 0 {
                    return None;
                }
                (name, Some(tick))
            }
            None => (spec, None),
        };
        Some(Self {
            scene: GameScene::parse(name)?,
            tick,
        })
    }
}

impl fmt::Display for GameSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.scene.fmt(f)?;
        if let Some(tick) = self.tick {
            write!(f, "@{tick}")?;
        }
        Ok(())
    }
}

/// Unsigned decimal without sign, prefix or leading `+`.
fn parse_decimal(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests;
