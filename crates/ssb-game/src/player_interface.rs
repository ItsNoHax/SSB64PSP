//! `ifCommonPlayerTag*`, `ifCommonPlayerArrows*`, `ifCommonPlayerMagnify*`
//! and `ifCommonItemArrow*`.
//! Screen coordinates are original 320x240 pixels, with projected Y up.

use crate::{
    camera::Camera,
    fighter::Fighter,
    status::{AnyStatus, Status},
};
use ssb_engine::math::Vec3;

pub const TAG_COLORS: [[u8; 4]; 5] = [
    [0xED, 0x36, 0x36, 255],
    [0x4E, 0x4E, 0xE9, 255],
    [255, 0xDF, 0x1A, 255],
    [0x4E, 0xB9, 0x4E, 255],
    [0xAC, 0xAC, 0xAC, 255],
];
pub const MAGNIFY_COLORS: [[u8; 4]; 5] = [
    [0xEF, 0x0D, 0x17, 255],
    [0, 0, 255, 255],
    [255, 0xE1, 0, 255],
    [0, 255, 0, 255],
    [255; 4],
];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FighterInterface {
    pub tag_wait: u16,
    pub tag_hide: bool,
    pub tag_bossend: bool,
    pub magnify_ignore: bool,
    pub control_disable: bool,
}

pub fn in_bounds((x, y): (f32, f32)) -> bool {
    (-150.0..=150.0).contains(&x) && (-110.0..=110.0).contains(&y)
}

pub fn tag_position(
    f: &Fighter,
    camera: &Camera,
    zoom_base: f32,
    size: [u16; 2],
) -> Option<(f32, f32)> {
    if f.interface.tag_hide
        || f.interface.tag_bossend
        || (f.interface.tag_wait != 1 && camera.eye.z <= 6000.0)
    {
        return None;
    }
    let xy = camera.project(f.pos + Vec3::new(0.0, zoom_base, 0.0));
    let (cx, cy) = camera.viewport_center();
    camera.in_viewport(xy).then(|| {
        (
            ((cx + xy.0 - f32::from(size[0]) * 0.5) as i32) as f32,
            ((cy - xy.1 - f32::from(size[1])) as i32) as f32,
        )
    })
}

/// `ifCommonItemArrowSetAttr`: the pickup arrow's primitive colour.
pub const ITEM_ARROW_COLOR: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];

/// `ifCommonItemArrowProcDisplay`: the pickup arrow's top-left corner, 100
/// units above the item's map collision top, while it shows and projects
/// inside the camera's bounds.
pub fn item_arrow_position(
    item: &crate::item::Item,
    camera: &Camera,
    size: [u16; 2],
) -> Option<(f32, f32)> {
    if !item.is_arrow_shown() {
        return None;
    }
    let xy = camera.project(item.pos + Vec3::new(0.0, item.coll.top + 100.0, 0.0));
    let (cx, cy) = camera.viewport_center();
    camera.in_viewport(xy).then(|| {
        (
            ((cx + xy.0 - f32::from(size[0]) * 0.5) as i32) as f32,
            ((cy - xy.1 - f32::from(size[1])) as i32) as f32,
        )
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct View {
    /// Main-camera culling, even while magnifiers are disabled.
    pub offscreen: bool,
    pub eligible: bool,
    pub direction: (f32, f32),
    /// Arrows use the shifted visibility probe, not the magnifier anchor.
    pub arrow: u8,
}

pub fn view(f: &Fighter, camera: &Camera, cam_offset_y: f32) -> View {
    if f.is_invisible
        || matches!(
            f.status.status,
            AnyStatus::Common(Status::DeadUpStar | Status::DeadUpFall)
        )
    {
        return View::default();
    }
    let mut probe = f.pos + Vec3::new(0.0, cam_offset_y, 0.0);
    let toward = camera.at - probe;
    if cam_offset_y < toward.length() {
        probe += toward.normalized() * cam_offset_y;
    }
    let xy = camera.project(probe);
    if camera.in_viewport(xy) {
        return View::default();
    }
    View {
        offscreen: true,
        eligible: !f.interface.magnify_ignore && !f.dead.is_rebirth,
        direction: camera.project(f.pos + Vec3::new(0.0, 300.0, 0.0)),
        arrow: arrow_flag(xy),
    }
}

pub fn arrow_flag((x, y): (f32, f32)) -> u8 {
    if x.abs() > y.abs() {
        if x > 0.0 {
            2
        } else {
            1
        }
    } else {
        0
    }
}

/// `gmCameraPlayerMagnifyFuncMatrix`: projected 600 units / 18, capped at 3.
pub fn magnify_scale(camera: &Camera) -> f32 {
    let dist = (camera.eye - camera.at).length();
    let mut mini = *camera;
    mini.eye = Vec3::new(0.0, 300.0, dist);
    mini.at = Vec3::new(0.0, 300.0, 0.0);
    (mini.project(Vec3::new(0.0, 900.0, 0.0)).1 / 18.0).min(3.0)
}

/// Ray intersection with the inset viewport (`ifCommonPlayerMagnifyGetPosition`).
pub fn magnify_position(xy: (f32, f32), scale: f32) -> (f32, f32) {
    magnify_position_in(xy, scale, (150.0, 110.0))
}

/// [`magnify_position`] in a viewport `half` its size across and up
/// (`gGMCameraStruct.viewport_width / 2`, `viewport_height / 2`).
pub fn magnify_position_in((x, y): (f32, f32), scale: f32, half: (f32, f32)) -> (f32, f32) {
    let right = half.0 - 20.0 * scale - 5.0;
    let up = half.1 - 20.0 * scale;
    if x == 0.0 {
        return (0.0, if y > 0.0 { up } else { -up });
    }
    let slope = y / x;
    if slope > up / right || slope < -up / right {
        let edge = if y > 0.0 { up } else { -up };
        ((edge * x / y).clamp(-right, right), edge)
    } else {
        let edge = if x > 0.0 { right } else { -right };
        (edge, (edge * y / x).clamp(-up, up))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Arrow {
    pub status: u8,
    /// AddAnim plays once, then ProcUpdate plays again on the rising edge.
    pub ticks: u32,
    pub epoch: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Interface {
    /// Last main-camera display, read by the next interface process.
    pub views: [View; 4],
    pub arrows: [Arrow; 2],
    pub sound_wait: u8,
}

impl Interface {
    /// `ifCommonPlayerArrowsFuncRun`, before priority-5 arrow animation
    /// processes. Plays `nSYAudioFGMMagnify` every 30 ticks while an arrow
    /// shows, and returns whether it played.
    pub fn tick(&mut self, magnify_display: bool) -> bool {
        let mut flags = 0;
        if magnify_display {
            for v in self.views.iter().filter(|v| v.offscreen && v.eligible) {
                flags |= arrow_flag(v.direction);
            }
            for (i, a) in self.arrows.iter_mut().enumerate() {
                a.status = if flags & (1 << i) == 0 {
                    0
                } else if a.status == 0 {
                    1
                } else {
                    2
                };
            }
        }
        for a in &mut self.arrows {
            if a.status == 1 {
                a.ticks = 2;
                a.epoch = a.epoch.wrapping_add(1);
            } else if a.status != 0 {
                a.ticks = a.ticks.wrapping_add(1);
            }
        }
        if flags == 0 {
            self.sound_wait = 0;
            return false;
        }
        let sound = self.sound_wait == 0;
        if sound {
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMMagnify);
            self.sound_wait = 30;
        }

        self.sound_wait -= 1;
        sound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fighter::FighterKind, status};

    #[test]
    fn edge_intersection_uses_source_insets_and_strict_diagonal() {
        assert_eq!(magnify_position((1000.0, 0.0), 1.0), (125.0, 0.0));
        assert_eq!(magnify_position((0.0, -1000.0), 3.0), (0.0, -50.0));
        assert_eq!(magnify_position((-1000.0, 1000.0), 1.0), (-90.0, 90.0));
        assert_eq!(arrow_flag((100.0, 100.0)), 0);
        assert!(in_bounds((150.0, -110.0)));
        assert!(!in_bounds((150.01, 0.0)));
    }

    #[test]
    fn arrow_restart_double_play_and_thirty_tick_sound() {
        let mut i = Interface::default();
        i.views[0] = View {
            offscreen: true,
            eligible: true,
            direction: (-200.0, 0.0),
            ..View::default()
        };
        assert!(i.tick(true));
        assert_eq!(i.arrows[0].ticks, 2);
        for _ in 0..29 {
            assert!(!i.tick(true));
        }
        assert!(i.tick(true));
        i.views[0].eligible = false;
        assert!(!i.tick(true));
        assert_eq!(i.arrows[0].status, 0);
        i.views[0].eligible = true;
        assert!(i.tick(true));
        assert_eq!(i.arrows[0].epoch, 2);
    }

    #[test]
    fn tags_use_wait_or_far_camera_and_status_gates() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        let mut c = Camera::default();
        c.eye = Vec3::new(0.0, 0.0, 5000.0);
        c.at = Vec3::ZERO;
        assert!(tag_position(&f, &c, 800.0, [24, 24]).is_none());
        f.interface.tag_wait = 1;
        assert!(tag_position(&f, &c, 800.0, [24, 24]).is_some());
        status::set_wait(&mut f);
        assert_eq!(f.interface.tag_wait, 120);
        status::set_status(&mut f, Status::Fall, 0.0, status::StatusTiming::unknown());
        assert_eq!(f.interface.tag_wait, 0);
        f.interface.tag_wait = 1;
        f.interface.tag_hide = true;
        assert!(tag_position(&f, &c, 800.0, [24, 24]).is_none());
    }

    #[test]
    fn shifted_probe_culls_but_star_and_invisible_do_not_magnify() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        let c = Camera::default();
        f.pos.x = 20000.0;
        assert!(view(&f, &c, 300.0).offscreen);
        f.interface.magnify_ignore = true;
        assert!(!view(&f, &c, 300.0).eligible);
        f.is_invisible = true;
        assert!(!view(&f, &c, 300.0).offscreen);
        f.is_invisible = false;
        f.status.status = Status::DeadUpStar.into();
        assert!(!view(&f, &c, 300.0).offscreen);
    }

    #[test]
    fn tag_countdown_stops_in_hitlag_and_while_controls_are_locked() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        status::set_wait(&mut f);
        let surfaces = || core::iter::empty::<crate::weapon::MapSurface>();
        f.interface.control_disable = true;
        f.tick_interrupt(&surfaces);
        assert_eq!(f.interface.tag_wait, 120);
        f.interface.control_disable = false;
        f.hitlag = 2;
        f.tick_interrupt(&surfaces);
        assert_eq!(f.interface.tag_wait, 120);
        f.hitlag = 0;
        f.tick_interrupt(&surfaces);
        assert_eq!(f.interface.tag_wait, 119);
        f.interface.tag_wait = 1;
        f.tick_interrupt(&surfaces);
        assert_eq!(f.interface.tag_wait, 1);
    }

    #[test]
    fn tag_preservation_and_crouch_entry_use_the_source_status_rules() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.interface.tag_wait = 10;
        status::set_any_status_preserve(
            &mut f,
            Status::DamageFall.into(),
            0.0,
            status::StatusTiming::unknown(),
            status::Preserve {
                playertag: true,
                ..Default::default()
            },
        );
        assert_eq!(f.interface.tag_wait, 10);
        status::set_squat(&mut f);
        assert_eq!(f.interface.tag_wait, 0);
        status::set_status(&mut f, Status::Entry, 0.0, status::StatusTiming::unknown());
        assert!(f.interface.tag_hide);
        status::set_status(
            &mut f,
            Status::DeadUpStar,
            0.0,
            status::StatusTiming::unknown(),
        );
        assert!(!f.interface.tag_hide);
        status::set_status(
            &mut f,
            Status::DeadDown,
            0.0,
            status::StatusTiming::unknown(),
        );
        assert!(f.interface.tag_hide);
        status::set_wait(&mut f);
        assert!(!f.interface.tag_hide);
    }
}
