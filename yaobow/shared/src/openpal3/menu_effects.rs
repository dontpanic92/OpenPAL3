use crosscom::ComRc;
use radiance::comdef::IUiHost;

use super::comdef::IPal3UiAtlas;
use fileformats::pal3::gdb::ItemDefinition;

#[derive(Clone, Copy)]
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ax: f32,
    ay: f32,
    color: [u8; 3],
    alpha: u8,
    rising: bool,
    active: bool,
}

impl Spark {
    // Original 0x4b5bb0 (style 8) and 0x4b5830 (style 7), once per paint.
    fn advance(&mut self, random: &mut impl FnMut() -> u32) {
        if self.rising {
            self.x += self.vx;
            self.vx += self.ax * 0.8;
        } else {
            let jitter = self.vx as i32;
            self.x += (random() % (jitter * 2) as u32) as f32 - jitter as f32;
            self.vx += self.ax;
        }
        if self.x > 64. {
            self.x = 64. - (random() % 64) as f32;
        }
        if self.x < 0. {
            self.x = (random() % 64) as f32;
        }
        if self.rising {
            self.y = 86. - self.vy;
            self.active = self.y > 16.;
            self.vy += self.ay * 0.8;
        } else {
            self.y += self.vy;
            self.active = self.y < 70.;
            self.vy = (self.vy + self.ay) * 0.8;
        }
        self.alpha = self.alpha.saturating_sub(1);
        self.active &= self.alpha > 0;
    }
}

#[derive(Default)]
pub(crate) struct ItemEffects {
    roles: [Vec<Spark>; 5],
    seed: u32,
}

impl ItemEffects {
    pub fn clear(&mut self) {
        for role in &mut self.roles {
            role.clear();
        }
    }

    // Original 0x49fbe0: twelve pairs, 32x32, resource-specific BGRA tint.
    pub fn spawn(&mut self, role: usize, item: &ItemDefinition) {
        let color = if item.amounts[0] != 0 {
            [168u8, 168, 28]
        } else if item.amounts[1] != 0 {
            [88, 168, 28]
        } else if item.amounts[2] != 0 {
            [28, 88, 168]
        } else {
            [138, 84, 224]
        };
        let Some(sparks) = self.roles.get_mut(role) else {
            return;
        };
        sparks.clear();
        let mut random = || {
            self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
            (self.seed >> 16) & 0x7fff
        };
        for _ in 0..12 {
            let blue = color[2] + (random() % 10) as u8;
            let green = color[1] + (random() % 10) as u8;
            let red = color[0] + (random() % 10) as u8;
            random();
            let alpha = (120 + random() % 135) as u8;
            for _ in 0..3 {
                random();
            }
            let vx = (1 + random() % 2) as f32;
            let vy = (1 + random() % 2) as f32;
            let ax = (2 + random() % 2) as f32;
            let ay = (2 + random() % 2) as f32;
            for rising in [true, false] {
                sparks.push(Spark {
                    x: 32.,
                    y: 0.,
                    vx,
                    vy,
                    ax,
                    ay,
                    color: [red, green, blue],
                    alpha,
                    rising,
                    active: true,
                });
            }
        }
    }

    pub fn paint(
        &mut self,
        ui: ComRc<IUiHost>,
        atlas: ComRc<IPal3UiAtlas>,
        role: usize,
        x: f32,
        y: f32,
        scale: f32,
    ) {
        let Some(sparks) = self.roles.get_mut(role) else {
            return;
        };
        if !sparks.iter().any(|spark| spark.active) {
            return;
        }
        let sprite = atlas.sprite("ui/GameMainUI/Par_Effect/spark.tga");
        let mut random = || {
            self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
            (self.seed >> 16) & 0x7fff
        };
        for spark in sparks.iter_mut().filter(|spark| spark.active) {
            if let Some(sprite) = &sprite {
                ui.image_rect_additive(
                    sprite.com_id(),
                    x + (spark.x - 16.).floor() * scale,
                    y + (spark.y - 16.).floor() * scale,
                    x + (spark.x + 16.).floor() * scale,
                    y + (spark.y + 16.).floor() * scale,
                    sprite.u0(),
                    sprite.v0(),
                    sprite.u1(),
                    sprite.v1(),
                    spark.color[0] as f32 / 255.,
                    spark.color[1] as f32 / 255.,
                    spark.color[2] as f32 / 255.,
                    spark.alpha as f32 / 255.,
                );
            }
            spark.advance(&mut random);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spark_pairs_follow_original_zero_rng_probe_and_expire() {
        let spark = Spark {
            x: 32.,
            y: 0.,
            vx: 1.,
            vy: 1.,
            ax: 2.,
            ay: 2.,
            color: [168, 168, 28],
            alpha: 120,
            rising: true,
            active: true,
        };
        let mut pair = [
            spark,
            Spark {
                rising: false,
                ..spark
            },
        ];
        // Local coordinates from the original executable probe at portrait (711,72).
        for expected in [
            [[16, -16], [16, -16]],
            [[17, 69], [15, -15]],
            [[19, 67], [12, -13]],
            [[23, 65], [7, -10]],
        ] {
            for (spark, expected) in pair.iter_mut().zip(expected) {
                assert_eq!(
                    [
                        (spark.x - 16.).floor() as i32,
                        (spark.y - 16.).floor() as i32
                    ],
                    expected
                );
                spark.advance(&mut || 0);
            }
        }
        for _ in 0..120 {
            for spark in pair.iter_mut().filter(|spark| spark.active) {
                spark.advance(&mut || 0);
            }
        }
        assert!(pair.iter().all(|spark| !spark.active));
    }
}
