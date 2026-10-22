//! Opt-in Rust-owned combo rows. Assets remain shared; only live rows own instances.
use crate::{scene::SceneManager as Scene, xr_round::ComboEvent};
use glam::Vec3;
use serde_json::{json, Value};

// A full authored minute has 140 targets; cap malformed external counts as well.
const MAX_CHAIN_ICONS: usize = 144;
const CHAIN_PULSE_SECONDS: f32 = 0.76;
const STOCK_FRAME_EMISSION: f32 = 8.;
const RETIRE_SECONDS: f32 = 0.95;

struct RetiredPart {
    id: u32,
    start: Vec3,
    scale: Vec3,
    target: Vec3,
}
struct RetiredRow {
    age: f32,
    parts: Vec<RetiredPart>,
    earned_bonus: u64,
    earned_points: u64,
    score_label: Option<u32>,
}

fn chain_arrival(progress: f32) -> (f32, f32) {
    let t = progress.clamp(0., 1.);
    if t < 0.45 {
        let remaining = (1. - t / 0.45).powi(3);
        (0.55 + 0.80 * (1. - remaining), remaining)
    } else if t < 0.72 {
        let u = (t - 0.45) / 0.27;
        let eased = u * u * (3. - 2. * u);
        (1.35 - 0.60 * eased, 0.)
    } else {
        let remaining = (1. - (t - 0.72) / 0.28).powi(3);
        (1. - 0.25 * remaining, 0.)
    }
}

fn chain_landing_glow(progress: f32) -> f32 {
    let t = if progress < 0.50 {
        (progress - 0.38) / 0.12
    } else {
        (1. - progress) / 0.50
    }
    .clamp(0., 1.);
    t * t * (3. - 2. * t)
}

struct Symbol {
    kind: String,
    asset: u32,
    variant: u32,
}
pub struct ComboDisplay {
    notice_seconds: f32,
    stack_score_panel: Option<u32>,
    stock_score_panels: Option<[u32; 2]>,
    retired: Option<RetiredRow>,
    pending_bonus: u64,
    stack_score_pulse: f32,
    chain_pulse: [f32; 2],
    pub chain_mode: bool,
    chain_rows: [Option<Row>; 2],
    symbols: Vec<Symbol>,
    frame_asset: u32,
    origin: Vec3,
    spacing: f32,
    row_gap: f32,
    fruit_scale: f32,
    frame_scale: Vec3,
    seconds: f32,
    pub timer_panel: u32,
    pub score_panel: u32,
    awards: [u32; 3],
    current: Option<Row>,
    history: Vec<Row>,
}
struct Row {
    kind: String,
    count: u32,
    success: bool,
    points: u32,
    age: f32,
    icons: Vec<u32>,
    frames: Vec<u32>,
}
impl Row {
    fn new(kind: String, count: u32, success: bool, points: u32) -> Self {
        Self {
            kind,
            count,
            success,
            points,
            age: 0.,
            icons: vec![],
            frames: vec![],
        }
    }
    fn remove(&mut self, scene: &mut Scene) {
        for id in self.icons.drain(..).chain(self.frames.drain(..)) {
            scene.remove_object(id);
        }
    }
    fn value(&self, seconds: f32, label: u32) -> Value {
        json!({"kind":self.kind,"count":self.count,"success":self.success,"points":self.points,
            "age":self.age,"opacity":(1.-self.age/seconds).clamp(0.,1.),"icons":self.icons,"frames":self.frames,"label":label})
    }
}
impl ComboDisplay {
    pub fn configure(scene: &Scene, input: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(input).ok()?;
        let chain_mode = match v.get("mode").and_then(Value::as_str) {
            None if v.get("mode").is_none() => false,
            Some("legacy") => false,
            Some("chains") => true,
            _ => return None,
        };
        let id = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
        let vector = |key: &str, default: [f32; 3]| -> Option<Vec3> {
            let Some(v) = v.get(key) else {
                return Some(Vec3::from(default));
            };
            let a = v.as_array()?;
            if a.len() != 3 {
                return None;
            }
            let out = Vec3::new(
                a[0].as_f64()? as f32,
                a[1].as_f64()? as f32,
                a[2].as_f64()? as f32,
            );
            out.is_finite().then_some(out)
        };
        let number = |key: &str, default: f32| -> Option<f32> {
            let n = v
                .get(key)
                .map_or(Some(default), |n| n.as_f64().map(|n| n as f32))?;
            (n.is_finite() && n > 0. && n <= 100.).then_some(n)
        };
        let mut symbols = vec![];
        for s in v["symbols"].as_array()? {
            let kind = s["kind"].as_str()?.to_owned();
            let asset = id(&s["asset"])?;
            let variant = id(&s["variant"])?;
            if kind.is_empty()
                || symbols.iter().any(|s: &Symbol| s.kind == kind)
                || !scene
                    .mesh_variants
                    .get(&asset)
                    .is_some_and(|a| a.registered && (variant as usize) < a.variants.len())
            {
                return None;
            }
            symbols.push(Symbol {
                kind,
                asset,
                variant,
            });
        }
        if symbols.is_empty() || symbols.len() > 64 {
            return None;
        }
        let frame_asset = id(&v["frameAsset"])?;
        if !scene
            .mesh_variants
            .get(&frame_asset)
            .is_some_and(|a| a.registered)
        {
            return None;
        }
        let timer_panel = id(&v["timerPanel"])?;
        let score_panel = id(&v["scorePanel"])?;
        let a = v["awardPanels"].as_array()?;
        if a.len() != 3 {
            return None;
        }
        let awards = [id(&a[0])?, id(&a[1])?, id(&a[2])?];
        let panels = [timer_panel, score_panel, awards[0], awards[1], awards[2]];
        for (i, p) in panels.iter().enumerate() {
            let expected = match i {
                0 => crate::info_panel::PanelKind::Countdown,
                1 => crate::info_panel::PanelKind::Score,
                _ => crate::info_panel::PanelKind::ComboAward,
            };
            if panels[..i].contains(p)
                || !scene.info_panels.get(p).is_some_and(|panel| {
                    panel.kind == expected
                        || (i >= 2 && panel.kind == crate::info_panel::PanelKind::DebugText)
                })
            {
                return None;
            }
        }
        let stack_score_panel = match v.get("stackScorePanel") {
            None => None,
            Some(value) => {
                let id = id(value)?;
                if panels.contains(&id)
                    || !scene
                        .info_panels
                        .get(&id)
                        .is_some_and(|p| p.kind == crate::info_panel::PanelKind::ComboAward)
                {
                    return None;
                }
                Some(id)
            }
        };
        let stock_score_panels = match v.get("stockScorePanels") {
            None => None,
            Some(value) => {
                let values = value.as_array()?;
                if values.len() != 2 {
                    return None;
                }
                let ids = [id(&values[0])?, id(&values[1])?];
                if ids[0] == ids[1]
                    || ids.iter().any(|id| {
                        panels.contains(id)
                            || stack_score_panel == Some(*id)
                            || !scene
                                .info_panels
                                .get(id)
                                .is_some_and(|p| p.kind == crate::info_panel::PanelKind::ComboAward)
                    })
                {
                    return None;
                }
                Some(ids)
            }
        };
        Some(Self {
            stack_score_panel,
            stock_score_panels,
            retired: None,
            pending_bonus: 0,
            stack_score_pulse: 0.,
            notice_seconds: 0.,
            chain_pulse: [0.; 2],
            chain_mode,
            chain_rows: [None, None],
            symbols,
            frame_asset,
            origin: vector("origin", [0., 1.5, -2.])?,
            spacing: number("spacing", 0.4)?,
            row_gap: number("rowGap", 0.4)?,
            fruit_scale: number("fruitScale", 0.1)?,
            frame_scale: vector("frameScale", [0.15, 1., 1.])?,
            seconds: number("historySeconds", 2.)?,
            timer_panel,
            score_panel,
            awards,
            current: None,
            history: vec![],
        })
    }
    pub fn clear(&mut self, scene: &mut Scene) {
        self.clear_retired(scene);
        self.notice_seconds = 0.;
        self.pending_bonus = 0;
        self.stack_score_pulse = 0.;
        self.chain_pulse = [0.; 2];
        for row in self.chain_rows.iter_mut() {
            if let Some(mut row) = row.take() {
                row.remove(scene);
            }
        }
        if let Some(mut row) = self.current.take() {
            row.remove(scene);
        }
        for row in &mut self.history {
            row.remove(scene);
        }
        self.history.clear();
        for id in self
            .awards
            .into_iter()
            .chain(self.stack_score_panel)
            .chain(self.stock_score_panels.into_iter().flatten())
        {
            if let Some(panel) = scene.info_panels.get_mut(&id) {
                panel.update(vec![("", String::new())]);
            }
            scene.set_opacity(id, 0., 8., 0., -1.);
        }
    }
    pub fn sync_chains(
        &mut self,
        scene: &mut Scene,
        chains: &[Option<crate::xr_round::FruitChain>; 2],
    ) {
        let mut hit = false;
        for (index, chain) in chains.iter().enumerate() {
            let kind = chain.as_ref().map_or("", |c| c.kind.as_str());
            let count = chain.as_ref().map_or(0, |c| c.count);
            if count > 0
                && self.chain_rows[index]
                    .as_ref()
                    .is_some_and(|r| r.count > 0 && r.kind != kind)
            {
                let old = self.chain_rows[index].take().unwrap();
                self.retire_row(scene, old, index);
            }
            let row =
                self.chain_rows[index].get_or_insert_with(|| Row::new(String::new(), 0, false, 0));
            let new_hit = count > 0 && (row.kind != kind || count > row.count);
            hit |= new_hit;
            if count == 0 {
                self.chain_pulse[index] = 0.;
            } else if new_hit {
                self.chain_pulse[index] = CHAIN_PULSE_SECONDS;
            }
            if row.kind != kind {
                for id in row.icons.drain(..) {
                    scene.remove_object(id);
                }
                row.kind = kind.into();
            }
            row.count = count;
            row.points = chain.as_ref().map_or(0, |c| c.points);
            let frame_count = (count as usize).min(MAX_CHAIN_ICONS);
            while row.frames.len() > frame_count {
                if let Some(id) = row.frames.pop() {
                    scene.remove_object(id);
                }
            }
            while row.frames.len() < frame_count {
                let Ok(id) = scene.add_mesh_variant(self.frame_asset, 0) else {
                    break;
                };
                row.frames.push(id);
            }
            if let Some(symbol) = self.symbols.iter().find(|s| s.kind == kind) {
                while row.icons.len() > (count as usize).min(MAX_CHAIN_ICONS) {
                    if let Some(id) = row.icons.pop() {
                        scene.remove_object(id);
                    }
                }
                while row.icons.len() < (count as usize).min(MAX_CHAIN_ICONS) {
                    let Ok(id) = scene.add_mesh_variant(symbol.asset, symbol.variant) else {
                        break;
                    };
                    row.icons.push(id);
                }
            }
            let center = self.origin + Vec3::new(if index == 0 { -0.65 } else { 0.65 }, 0., 0.);
            let direction = if index == 0 { -1. } else { 1. };
            let step = self.spacing;
            let icon_scale = self.fruit_scale;
            for (ids, scale, icons) in [
                (&row.frames, self.frame_scale, false),
                (&row.icons, Vec3::splat(icon_scale), true),
            ] {
                for (slot, &id) in ids.iter().enumerate() {
                    scene.set_object_position(
                        id,
                        center
                            + Vec3::new(
                                direction * (slot as f32 * step - self.spacing),
                                0.,
                                if icons { 0.02 } else { 0. },
                            ),
                    );
                    scene.set_object_scale(id, scale);
                    scene.set_opacity(id, 1., 8., 0., -1.);
                    scene.set_emission(id, if icons { 3. } else { STOCK_FRAME_EMISSION });
                    scene.set_render_options(
                        id,
                        crate::scene::RenderOptions {
                            casts_shadow: false,
                            receives_shadow: false,
                            ..Default::default()
                        },
                    );
                }
            }
            let id = self.awards[index];
            if let Some(panel) = scene.info_panels.get_mut(&id) {
                panel.update(vec![(
                    "",
                    if count == 0 {
                        String::new()
                    } else {
                        count.to_string()
                    },
                )]);
            }
            let debug = scene
                .info_panels
                .get(&id)
                .is_some_and(|p| p.kind == crate::info_panel::PanelKind::DebugText);
            scene.set_object_position(
                id,
                center + Vec3::new(0., if debug { -0.14 } else { -0.23 }, 0.),
            );
            scene.set_opacity(id, if count == 0 { 0. } else { 1. }, 8., 0., -1.);
            scene.set_emission(id, if debug { 0. } else { 4. });
            self.sync_chain_pulse(scene, index);
            if let Some(ids) = self.stock_score_panels {
                let id = ids[index];
                let value = chain.as_ref().map_or(0, |c| c.total_points());
                if let Some(panel) = scene.info_panels.get_mut(&id) {
                    panel.update(vec![("", format!("+{value}"))]);
                }
                scene.set_object_position(
                    id,
                    self.origin + Vec3::new(if index == 0 { -0.32 } else { 0.32 }, -0.22, 0.04),
                );
                scene.set_opacity(id, if count > 0 { 1. } else { 0. }, 8., 0., -1.);
                self.sync_stock_score(scene, index);
            }
        }
        self.pending_bonus = chains
            .iter()
            .flatten()
            .fold(0_u64, |sum, c| sum.saturating_add(c.value()));
        if hit {
            self.stack_score_pulse = 0.42;
        }
        if chains.iter().all(Option::is_none) {
            self.stack_score_pulse = 0.;
        }
        if let Some(id) = self.stack_score_panel {
            if let Some(panel) = scene.info_panels.get_mut(&id) {
                panel.update(vec![("", format!("+{}", self.pending_bonus))]);
            }
            scene.set_object_position(id, self.origin + Vec3::new(0., 0., 0.04));
            scene.set_opacity(id, 1., 8., 0., -1.);
            self.sync_stack_score(scene);
        }
    }
    fn sync_stack_score(&self, scene: &mut Scene) {
        let Some(id) = self.stack_score_panel else {
            return;
        };
        let pulse = (self.stack_score_pulse / 0.42).powi(2);
        let sign = scene
            .objects()
            .get(&id)
            .map_or(1., |o| o.transform.scale.x.signum());
        let size = 1. + 0.12 * pulse;
        scene.set_object_scale(id, Vec3::new(sign * size, size, size));
        scene.set_emission(id, 4. + 4. * pulse);
    }
    fn sync_stock_score(&self, scene: &mut Scene, index: usize) {
        let Some(ids) = self.stock_score_panels else {
            return;
        };
        let id = ids[index];
        let pulse = (self.chain_pulse[index] / CHAIN_PULSE_SECONDS).powi(2);
        let sign = scene
            .objects()
            .get(&id)
            .map_or(1., |o| o.transform.scale.x.signum());
        scene.set_object_scale(
            id,
            Vec3::new(
                sign * (1. + 0.12 * pulse),
                1. + 0.12 * pulse,
                1. + 0.12 * pulse,
            ),
        );
        scene.set_emission(id, 4. + 4. * pulse);
    }
    fn clear_retired(&mut self, scene: &mut Scene) {
        if let Some(row) = self.retired.take() {
            for part in row.parts {
                scene.remove_object(part.id);
            }
        }
    }
    fn retire_row(&mut self, scene: &mut Scene, row: Row, index: usize) {
        // One outgoing row at most; transfer existing instances rather than clone them.
        self.clear_retired(scene);
        let center = scene
            .objects()
            .get(&self.score_panel)
            .map_or(self.origin + Vec3::Y * 0.24, |o| {
                o.transform.position - Vec3::Y * 0.24
            });
        let count = row.icons.len().max(row.frames.len());
        let mut parts = Vec::with_capacity(row.icons.len() + row.frames.len() + 1);
        for (ids, icons) in [(&row.icons, true), (&row.frames, false)] {
            for (index, &id) in ids.iter().enumerate() {
                if let Some(object) = scene.objects().get(&id) {
                    parts.push(RetiredPart {
                        id,
                        start: object.transform.position,
                        scale: object.transform.scale,
                        target: center
                            + Vec3::new(
                                (index as f32 - (count.saturating_sub(1)) as f32 * 0.5)
                                    * self.spacing
                                    * 0.65,
                                0.,
                                if icons { 0.04 } else { 0.02 },
                            ),
                    });
                }
            }
        }
        let earned_bonus = crate::xr_round::FruitChain {
            kind: row.kind,
            count: row.count,
            points: row.points,
        }
        .value();
        let earned_points = u64::from(row.count)
            .saturating_mul(crate::xr_round::CHAIN_HIT_POINTS)
            .saturating_add(earned_bonus);
        let mut score_label = None;
        if earned_points > 0 {
            if let Ok(id) = scene.add_info_panel("combo-award", 0.42, 0.12) {
                if let Some(panel) = scene.info_panels.get_mut(&id) {
                    panel.update(vec![("", format!("+{earned_points}"))]);
                }
                let source = self
                    .stock_score_panels
                    .map(|ids| ids[index])
                    .unwrap_or(self.score_panel);
                let sign = scene
                    .objects()
                    .get(&source)
                    .map_or(1., |o| o.transform.scale.x.signum());
                let start =
                    self.origin + Vec3::new(if index == 0 { -0.32 } else { 0.32 }, -0.22, 0.06);
                let scale = Vec3::new(sign, 1., 1.);
                scene.set_object_position(id, start);
                scene.set_object_scale(id, scale);
                scene.set_emission(id, 4.);
                scene.set_render_options(
                    id,
                    crate::scene::RenderOptions {
                        casts_shadow: false,
                        receives_shadow: false,
                        ..Default::default()
                    },
                );
                parts.push(RetiredPart {
                    id,
                    start,
                    scale,
                    target: center + Vec3::new(0., -0.11, 0.06),
                });
                score_label = Some(id);
            }
        }
        self.retired = Some(RetiredRow {
            age: 0.,
            parts,
            earned_bonus,
            earned_points,
            score_label,
        });
    }
    fn advance_retired(&mut self, scene: &mut Scene, dt: f32) {
        let Some(row) = &mut self.retired else {
            return;
        };
        row.age += dt;
        if row.age >= RETIRE_SECONDS {
            self.clear_retired(scene);
            return;
        }
        let t = row.age / RETIRE_SECONDS;
        let travel = 1. - (1. - (t / 0.6).min(1.)).powi(3);
        let fade = ((t - 0.35) / 0.65).clamp(0., 1.);
        let opacity = (1. - fade).powi(2);
        for part in &row.parts {
            scene.set_object_position(
                part.id,
                part.start.lerp(part.target, travel) + Vec3::Y * (0.05 * fade),
            );
            scene.set_object_scale(
                part.id,
                part.scale * (1. - 0.35 * travel) * (1. - 0.65 * fade * fade),
            );
            scene.set_opacity(part.id, opacity, 8., 0., -1.);
        }
    }
    fn sync_chain_pulse(&self, scene: &mut Scene, index: usize) {
        let Some(row) = &self.chain_rows[index] else {
            return;
        };
        let Some(&id) = row.icons.last() else {
            return;
        };
        // Arrival -> overshoot -> squash -> settle; one local highlight per hit.
        let remaining = self.chain_pulse[index] / CHAIN_PULSE_SECONDS;
        let landing = chain_landing_glow(1. - remaining);
        let (size, flight) = chain_arrival(1. - remaining);
        let direction = if index == 0 { -1. } else { 1. };
        let step = self.spacing;
        let center = self.origin + Vec3::new(direction * 0.65, 0., 0.);
        let destination = center
            + Vec3::new(
                direction * ((row.icons.len() - 1) as f32 * step - self.spacing),
                0.,
                0.02,
            );
        scene.set_object_position(
            id,
            destination + Vec3::new(direction * 0.90, 0., 0.) * flight,
        );
        let scale = self.fruit_scale;
        scene.set_object_scale(id, Vec3::splat(scale * size));
        for &fruit in &row.icons {
            scene.set_emission(fruit, 3. + if fruit == id { 5. } else { 2.5 } * landing);
        }
        // Fruit accents fade after landing; earned frame glow remains at its baseline.
        if let Some(&frame) = row.frames.get(row.icons.len() - 1) {
            scene.set_object_position(
                frame,
                destination - Vec3::new(0., 0., 0.02)
                    + Vec3::new(direction * 0.90, 0., 0.) * flight,
            );
            scene.set_object_scale(frame, self.frame_scale * size);
        }
    }
    pub fn notice(&mut self, scene: &mut Scene, label: String) {
        if !self.chain_mode {
            return;
        }
        self.notice_seconds = 0.5;
        let id = self.awards[2];
        if let Some(panel) = scene.info_panels.get_mut(&id) {
            panel.update(vec![("", label)]);
        }
        scene.set_object_position(id, self.origin + Vec3::new(0., -0.56, 0.));
        scene.set_opacity(id, 1., 8., 0., -1.);
        scene.set_emission(id, 4.);
    }
    pub fn reset(&mut self, scene: &mut Scene) {
        if self.chain_mode {
            self.clear(scene);
            self.sync_chains(scene, &[None, None]);
            return;
        }
        if let Some(mut r) = self.current.take() {
            r.remove(scene);
        }
        for r in &mut self.history {
            r.remove(scene);
        }
        self.history.clear();
        self.sync(scene);
    }
    pub fn event(&mut self, scene: &mut Scene, event: ComboEvent) {
        if let Some((kind, count)) = event.failed {
            if let Some(mut row) = self.current.take() {
                row.remove(scene);
            }
            self.history.insert(0, Row::new(kind, count, false, 0));
        }
        if let Some(mut row) = self.current.take() {
            row.remove(scene);
        }
        if event.success {
            self.history
                .insert(0, Row::new(event.kind, event.count, true, event.points));
        } else {
            self.current = Some(Row::new(event.kind, event.count, false, 0));
        }
        while self.history.len() > 3 {
            self.history.pop().unwrap().remove(scene);
        }
        self.sync(scene);
    }
    pub fn advance(&mut self, scene: &mut Scene, dt: f32) {
        if self.chain_mode {
            if !dt.is_finite() || dt <= 0. {
                return;
            }
            self.advance_retired(scene, dt);
            if self.stack_score_pulse > 0. {
                self.stack_score_pulse = (self.stack_score_pulse - dt).max(0.);
                self.sync_stack_score(scene);
            }
            for index in 0..2 {
                if self.chain_pulse[index] > 0. {
                    self.chain_pulse[index] = (self.chain_pulse[index] - dt).max(0.);
                    self.sync_chain_pulse(scene, index);
                    self.sync_stock_score(scene, index);
                }
            }
            if self.notice_seconds > 0. {
                self.notice_seconds = (self.notice_seconds - dt).max(0.);
                if self.notice_seconds == 0. {
                    if let Some(panel) = scene.info_panels.get_mut(&self.awards[2]) {
                        panel.update(vec![("", String::new())]);
                    }
                    scene.set_opacity(self.awards[2], 0., 8., 0., -1.);
                }
            }
            return;
        }
        if !dt.is_finite() || dt <= 0. || self.history.is_empty() {
            return;
        }
        for r in &mut self.history {
            r.age += dt;
        }
        self.history.retain_mut(|r| {
            if r.age >= self.seconds {
                r.remove(scene);
                false
            } else {
                true
            }
        });
        self.sync(scene);
    }
    fn sync(&mut self, scene: &mut Scene) {
        for (row_index, row) in self
            .current
            .iter_mut()
            .map(|r| (0, r))
            .chain(self.history.iter_mut().enumerate().map(|(i, r)| (i + 1, r)))
        {
            let Some(symbol) = self.symbols.iter().find(|s| s.kind == row.kind) else {
                continue;
            };
            while row.icons.len() < row.count.min(3) as usize {
                let Ok(id) = scene.add_mesh_variant(symbol.asset, symbol.variant) else {
                    break;
                };
                row.icons.push(id);
            }
            while row.frames.len() < 3 {
                let Ok(id) = scene.add_mesh_variant(self.frame_asset, 0) else {
                    break;
                };
                row.frames.push(id);
            }
            let opacity = if row_index == 0 {
                1.
            } else {
                (1. - row.age / self.seconds).clamp(0., 1.)
            };
            for (ids, scale, emission, icons) in [
                (&row.icons, Vec3::splat(self.fruit_scale), 3., true),
                (&row.frames, self.frame_scale, 7., false),
            ] {
                for (slot, &id) in ids.iter().enumerate() {
                    scene.set_object_position(
                        id,
                        self.origin
                            + Vec3::new(
                                (slot as f32 - 1.) * self.spacing,
                                -(row_index as f32) * self.row_gap,
                                0.,
                            ),
                    );
                    scene.set_object_scale(id, scale);
                    let unearned = !icons && slot >= row.count as usize;
                    scene.set_opacity(id, opacity * if unearned { 0.22 } else { 1. }, 8., 0., -1.);
                    scene.set_emission(
                        id,
                        emission * opacity * opacity * if unearned { 0.25 } else { 1. },
                    );
                    scene.set_render_options(
                        id,
                        crate::scene::RenderOptions {
                            casts_shadow: false,
                            receives_shadow: false,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        for (index, id) in self.awards.iter().enumerate() {
            let row = self.history.get(index);
            let label = row.map_or(String::new(), |r| {
                if r.success {
                    format!("+{}", r.points)
                } else {
                    "PRERUSENO".into()
                }
            });
            if let Some(panel) = scene.info_panels.get_mut(id) {
                panel.update(vec![("", label)]);
            }
            let opacity = row.map_or(0., |r| (1. - r.age / self.seconds).clamp(0., 1.));
            scene.set_opacity(*id, opacity, 8., 0., -1.);
            let debug = scene
                .info_panels
                .get(id)
                .is_some_and(|p| p.kind == crate::info_panel::PanelKind::DebugText);
            scene.set_emission(*id, if debug { 0. } else { 4. * opacity * opacity });
        }
    }
    pub fn state(&self) -> String {
        if self.chain_mode {
            let chains: Vec<Value> = self.chain_rows.iter().enumerate().map(|(i, row)| {
                let r = row.as_ref();
                json!({"kind":r.map_or("", |r|r.kind.as_str()),"count":r.map_or(0,|r|r.count),"points":r.map_or(0,|r|r.points),
                    "earnedPoints":r.map_or(0,|r|crate::xr_round::FruitChain{kind:r.kind.clone(),count:r.count,points:r.points}.total_points()),
                    "potential":r.map_or(0,|r|crate::xr_round::FruitChain{kind:r.kind.clone(),count:r.count,points:r.points}.value()),
                    "icons":r.map(|r|&r.icons),"frames":r.map(|r|&r.frames),"label":self.awards[i]})
            }).collect();
            return json!({"mode":"chains","pendingBonus":self.pending_bonus,"stackScorePanel":self.stack_score_panel,"stockScorePanels":self.stock_score_panels,"retiring":self.retired.as_ref().map(|r|json!({"age":r.age,"earnedBonus":r.earned_bonus,"earnedPoints":r.earned_points,"scoreLabel":r.score_label,"objects":r.parts.iter().map(|p|p.id).collect::<Vec<_>>()})),"current":null,"history":[],"chains":chains})
                .to_string();
        }
        json!({"current":self.current.as_ref().map(|r|r.value(self.seconds,0)),"history":self.history.iter().enumerate().map(|(i,r)|r.value(self.seconds,self.awards[i])).collect::<Vec<_>>()}).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Scene, ComboDisplay) {
        let mut scene = Scene::default();
        let asset = scene
            .register_mesh_variants(
                &[0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.],
                &[0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
                &[1., 0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0.],
                false,
            )
            .unwrap();
        let timer = scene.add_info_panel("countdown", 1., 0.3).unwrap();
        let score = scene.add_info_panel("score", 1., 0.3).unwrap();
        let awards: Vec<_> = (0..3)
            .map(|_| scene.add_info_panel("combo-award", 1., 0.2).unwrap())
            .collect();
        let config = json!({"symbols":[{"kind":"A","asset":asset,"variant":0},{"kind":"B","asset":asset,"variant":0},{"kind":"W","asset":asset,"variant":0}],"frameAsset":asset,"timerPanel":timer,"scorePanel":score,"awardPanels":awards});
        let display = ComboDisplay::configure(&scene, &config.to_string()).unwrap();
        (scene, display)
    }
    #[test]
    fn combo_display_mismatch_wild_success_fade_and_reset() {
        let (mut scene, mut display) = fixture();
        let mut score = crate::xr_round::ComboScore::default();
        for (id, kind, wild) in [
            (1, "A", false),
            (2, "W", true),
            (3, "B", false),
            (4, "W", true),
            (5, "B", false),
        ] {
            assert!(score.register(id, kind, 10, wild));
            display.event(&mut scene, score.hit_event(id).unwrap());
            if id == 2 {
                let row = display.current.as_ref().unwrap();
                assert_eq!(
                    (&*row.kind, row.count, row.icons.len(), row.frames.len()),
                    ("A", 2, 2, 3)
                );
            }
            if id == 3 {
                assert_eq!(
                    (
                        display.history[0].kind.as_str(),
                        display.history[0].count,
                        display.history[0].success
                    ),
                    ("A", 2, false)
                );
            }
        }
        assert!(display.current.is_none());
        assert_eq!(score.score, 10);
        assert_eq!(
            (
                display.history[0].kind.as_str(),
                display.history[0].points,
                display.history[0].success
            ),
            ("B", 10, true)
        );
        assert!(score.hit_event(5).is_none());
        let ids = display
            .history
            .iter()
            .flat_map(|r| r.icons.iter().chain(&r.frames))
            .copied()
            .collect::<Vec<_>>();
        display.advance(&mut scene, 1.);
        assert_eq!(display.history[0].value(2., 0)["opacity"], 0.5);
        for row in &display.history {
            for (slot, id) in row.icons.iter().enumerate() {
                assert_eq!(
                    scene.objects()[id].opacity,
                    if slot >= row.count as usize {
                        0.11
                    } else {
                        0.5
                    }
                );
            }
            assert_eq!(row.icons.len(), row.count as usize);
            for (slot, id) in row.frames.iter().enumerate() {
                assert_eq!(
                    scene.objects()[id].opacity,
                    if slot >= row.count as usize {
                        0.11
                    } else {
                        0.5
                    }
                );
            }
        }
        display.advance(&mut scene, 1.);
        assert!(display.history.is_empty());
        for id in ids {
            assert!(!scene.object_exists(id));
        }
        display.event(
            &mut scene,
            ComboEvent {
                failed: None,
                kind: "A".into(),
                count: 1,
                success: false,
                points: 0,
            },
        );
        display.reset(&mut scene);
        assert!(display.current.is_none() && display.history.is_empty());
        assert_eq!(scene.objects().len(), 5);
    }
    #[test]
    fn individual_bonuses_and_replaced_row_animate_then_release_instances() {
        let (mut scene, old) = fixture();
        scene.set_object_position(old.score_panel, Vec3::new(0., 1.90, -2.));
        let panels = [
            scene.add_info_panel("combo-award", 0.28, 0.11).unwrap(),
            scene.add_info_panel("combo-award", 0.28, 0.11).unwrap(),
        ];
        let config = json!({"mode":"chains","symbols":[{"kind":"A","asset":old.symbols[0].asset,"variant":0},{"kind":"B","asset":old.symbols[1].asset,"variant":0}],
            "frameAsset":old.frame_asset,"timerPanel":old.timer_panel,"scorePanel":old.score_panel,"awardPanels":old.awards,"stockScorePanels":panels});
        let mut display = ComboDisplay::configure(&scene, &config.to_string()).unwrap();
        let make = |kind: &str, count| {
            Some(crate::xr_round::FruitChain {
                kind: kind.into(),
                count,
                points: 10,
            })
        };
        display.sync_chains(&mut scene, &[make("A", 4), make("B", 5)]);
        let state: Value = serde_json::from_str(&display.state()).unwrap();
        assert_eq!(state["chains"][0]["potential"], 30);
        assert_eq!(state["chains"][0]["earnedPoints"], 70);
        assert_eq!(state["chains"][1]["earnedPoints"], 110);
        assert_eq!(state["chains"][1]["potential"], 60);
        assert!(scene.objects()[&panels[0]].transform.position.x < 0.);
        assert!(scene.objects()[&panels[1]].transform.position.x > 0.);
        display.advance(&mut scene, 0.8);
        let preserved = display.chain_rows[1].as_ref().unwrap().icons.clone();
        let old_ids = display.chain_rows[0].as_ref().unwrap().icons.clone();
        let start_y = scene.objects()[&old_ids[0]].transform.position.y;
        scene.set_object_scale(panels[0], Vec3::new(-1., 1., 1.));
        display.sync_chains(&mut scene, &[make("B", 1), make("B", 5)]);
        assert_eq!(display.chain_rows[1].as_ref().unwrap().icons, preserved);
        assert!(old_ids.iter().all(|id| scene.objects().contains_key(id)));
        assert!(!display.chain_rows[0]
            .as_ref()
            .unwrap()
            .icons
            .contains(&old_ids[0]));
        let retired = display.retired.as_ref().unwrap();
        assert_eq!(retired.earned_bonus, 30);
        assert_eq!(retired.earned_points, 70);
        let earned_label = retired.score_label.unwrap();
        assert!(scene.objects()[&earned_label].transform.scale.x < 0.);
        assert!(!scene.info_panels[&earned_label].mesh.vertices.is_empty());
        display.advance(&mut scene, 0.45);
        let moving = &scene.objects()[&old_ids[0]];
        assert!(
            moving.transform.position.y > start_y && moving.opacity < 1. && moving.opacity > 0.
        );
        let earned = &scene.objects()[&earned_label];
        assert!(earned.opacity > 0. && earned.opacity < 1. && earned.transform.scale.x < 0.);
        display.advance(&mut scene, 0.51);
        assert!(display.retired.is_none());
        assert!(
            !scene.objects().contains_key(&earned_label)
                && !scene.info_panels.contains_key(&earned_label)
        );
        assert!(old_ids.iter().all(|id| !scene.objects().contains_key(id)));
        display.sync_chains(&mut scene, &[make("A", 2), make("B", 5)]);
        assert!(display.retired.as_ref().unwrap().score_label.is_some());
        assert_eq!(display.retired.as_ref().unwrap().earned_points, 10);
        let outgoing = display
            .retired
            .as_ref()
            .unwrap()
            .parts
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>();
        display.sync_chains(&mut scene, &[make("B", 1), make("B", 5)]);
        assert!(outgoing.iter().all(|id| !scene.objects().contains_key(id)));
        let outgoing = display
            .retired
            .as_ref()
            .unwrap()
            .parts
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>();
        display.clear(&mut scene);
        assert!(display.retired.is_none());
        assert!(outgoing.iter().all(|id| !scene.objects().contains_key(id)));
        assert!(panels.iter().all(|id| scene.objects()[id].opacity == 0.));
        let mut invalid = config;
        invalid["stockScorePanels"] = json!([panels[0], panels[0]]);
        assert!(ComboDisplay::configure(&scene, &invalid.to_string()).is_none());
    }
    #[test]
    fn central_stack_bonus_tracks_both_stocks_pulses_and_clears() {
        let (mut scene, old) = fixture();
        let pending = scene.add_info_panel("combo-award", 0.28, 0.11).unwrap();
        let config = json!({"mode":"chains","symbols":[{"kind":"A","asset":old.symbols[0].asset,"variant":0},{"kind":"B","asset":old.symbols[1].asset,"variant":0}],
            "frameAsset":old.frame_asset,"timerPanel":old.timer_panel,"scorePanel":old.score_panel,"awardPanels":old.awards,"stackScorePanel":pending});
        let mut display = ComboDisplay::configure(&scene, &config.to_string()).unwrap();
        let mut chains = [
            Some(crate::xr_round::FruitChain {
                kind: "A".into(),
                count: 4,
                points: 10,
            }),
            Some(crate::xr_round::FruitChain {
                kind: "B".into(),
                count: 5,
                points: 20,
            }),
        ];
        display.sync_chains(&mut scene, &chains);
        assert_eq!(display.pending_bonus, 150);
        assert_eq!(
            scene.objects()[&pending].transform.position.x,
            display.origin.x
        );
        assert!(scene.objects()[&pending].transform.scale.x > 1.1);
        assert_eq!(scene.objects()[&pending].emission, 8.);
        display.advance(&mut scene, 0.5);
        assert_eq!(scene.objects()[&pending].transform.scale.x, 1.);
        assert_eq!(scene.objects()[&pending].emission, 4.);
        chains[0] = None;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(display.pending_bonus, 120);
        chains[1] = None;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(display.pending_bonus, 0);
        assert!(!scene.info_panels[&pending].mesh.vertices.is_empty());
        display.clear(&mut scene);
        assert_eq!(scene.objects()[&pending].opacity, 0.);
        assert!(scene.info_panels[&pending].mesh.vertices.is_empty());
        let mut invalid = config;
        invalid["stackScorePanel"] = serde_json::json!(old.score_panel);
        assert!(ComboDisplay::configure(&scene, &invalid.to_string()).is_none());
    }
    #[test]
    fn debug_stock_counters_keep_plain_font_and_zero_emission() {
        let (mut scene, old) = fixture();
        let counts = [
            scene.add_info_panel("debug-text", 0.18, 0.05).unwrap(),
            scene.add_info_panel("debug-text", 0.18, 0.05).unwrap(),
        ];
        let config = json!({"mode":"chains", "symbols":[{"kind":"A","asset":old.symbols[0].asset,"variant":0}],"frameAsset":old.frame_asset,
            "timerPanel":old.timer_panel,"scorePanel":old.score_panel,"awardPanels":[counts[0],counts[1],old.awards[2]]});
        let mut display = ComboDisplay::configure(&scene, &config.to_string()).unwrap();
        display.sync_chains(
            &mut scene,
            &[
                Some(crate::xr_round::FruitChain {
                    kind: "A".into(),
                    count: 5,
                    points: 10,
                }),
                None,
            ],
        );
        let panel = &scene.info_panels[&counts[0]];
        assert!(!panel.kind.neon());
        assert!(!panel.mesh.indices.is_empty());
        assert!(panel.height <= 0.05);
        assert_eq!(scene.objects()[&counts[0]].emission, 0.);
        display.advance(&mut scene, CHAIN_PULSE_SECONDS * 0.5);
        assert_eq!(scene.objects()[&counts[0]].emission, 0.);
        assert!(scene.objects()[&counts[1]].opacity == 0.);
        display.clear(&mut scene);
        assert_eq!(scene.objects()[&counts[0]].opacity, 0.);
    }
    #[test]
    fn combo_display_two_chains_remain_bounded_and_reset_cleanly() {
        let (mut scene, mut display) = fixture();
        display.chain_mode = true;
        display.reset(&mut scene);
        assert_eq!(scene.variant_instances.len(), 0);
        let mut chains = [
            Some(crate::xr_round::FruitChain {
                kind: "A".into(),
                count: 5,
                points: 10,
            }),
            Some(crate::xr_round::FruitChain {
                kind: "B".into(),
                count: 2,
                points: 20,
            }),
        ];
        display.sync_chains(&mut scene, &chains);
        assert_eq!(scene.variant_instances.len(), 14);
        let state: Value = serde_json::from_str(&display.state()).unwrap();
        assert_eq!(state["chains"][0]["count"], 5);
        assert_eq!(state["chains"][0]["potential"], 60);
        let icons = display.chain_rows[0].as_ref().unwrap().icons.clone();
        chains[0].as_mut().unwrap().count = 100;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(&display.chain_rows[0].as_ref().unwrap().icons[..5], &icons);
        assert_eq!(scene.variant_instances.len(), 204);
        display.advance(&mut scene, CHAIN_PULSE_SECONDS);
        for row in display.chain_rows.iter().flatten() {
            for id in &row.icons {
                assert!((scene.objects()[id].transform.scale.x - display.fruit_scale).abs() < 1e-6);
            }
            for id in &row.frames {
                assert!(
                    (scene.objects()[id].transform.scale.x - display.frame_scale.x).abs() < 1e-6
                );
            }
        }
        let hundred = display.chain_rows[0].as_ref().unwrap();
        assert!(
            scene.objects()[hundred.icons.last().unwrap()]
                .transform
                .position
                .x
                < -30.
        );

        chains[0].as_mut().unwrap().count = u32::MAX;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(scene.variant_instances.len(), 2 * MAX_CHAIN_ICONS + 4);
        chains[0].as_mut().unwrap().count = 4;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(scene.variant_instances.len(), 12);
        let row = display.chain_rows[0].as_ref().unwrap();
        assert_eq!(row.icons.len(), 4);
        assert!(
            scene.objects()[&row.icons[3]].transform.position.x
                < scene.objects()[&row.icons[0]].transform.position.x
        );
        chains[0] = None;
        display.sync_chains(&mut scene, &chains);
        assert_eq!(scene.variant_instances.len(), 4);
        display.reset(&mut scene);
        assert_eq!(scene.variant_instances.len(), 0);
        display.clear(&mut scene);
        assert!(scene.variant_instances.is_empty());
    }
    #[test]
    fn chain_rows_grow_outwards_and_newest_icon_pulses_once() {
        let (mut scene, mut display) = fixture();
        display.chain_mode = true;
        let chains = [
            Some(crate::xr_round::FruitChain {
                kind: "A".into(),
                count: 4,
                points: 10,
            }),
            Some(crate::xr_round::FruitChain {
                kind: "B".into(),
                count: 5,
                points: 20,
            }),
        ];
        display.sync_chains(&mut scene, &chains);
        for index in 0..2 {
            let row = display.chain_rows[index].as_ref().unwrap();
            let sign = if index == 0 { -1. } else { 1. };
            for pair in row.icons[..row.icons.len() - 1].windows(2) {
                let a = &scene.objects()[&pair[0]].transform.position;
                let b = &scene.objects()[&pair[1]].transform.position;
                assert!((b.x - a.x) * sign > 0.);
                assert_eq!(a.y, b.y);
            }
            let last = &scene.objects()[row.icons.last().unwrap()];

            assert!((last.transform.scale.x / (display.fruit_scale) - 0.55).abs() < 1e-5);
            let frame = &scene.objects()[row.frames.last().unwrap()];
            assert!((last.transform.position.x - frame.transform.position.x).abs() < 1e-6);
            assert!((last.transform.position.y - frame.transform.position.y).abs() < 1e-6);
            assert!((frame.transform.scale.x / (display.frame_scale.x) - 0.55).abs() < 1e-5);
            let target_x = display.origin.x
                + sign * (0.65 + (row.icons.len() - 1) as f32 * display.spacing - display.spacing);
            assert!((last.transform.position.x - target_x) * sign > 0.85);
            assert_eq!(last.emission, 3.);
            assert_eq!(frame.emission, STOCK_FRAME_EMISSION);
            assert_eq!(frame.opacity, 1.);
        }
        display.advance(&mut scene, CHAIN_PULSE_SECONDS * 0.45);
        let last = *display.chain_rows[1]
            .as_ref()
            .unwrap()
            .icons
            .last()
            .unwrap();

        let peak = &scene.objects()[&last];
        assert!((peak.transform.scale.x / (display.fruit_scale) - 1.35).abs() < 1e-5);
        assert!((peak.transform.position.y - display.origin.y).abs() < 1e-6);
        let frame = *display.chain_rows[1]
            .as_ref()
            .unwrap()
            .frames
            .last()
            .unwrap();
        assert!(
            (scene.objects()[&frame].transform.scale.x / (display.frame_scale.x) - 1.35).abs()
                < 1e-5
        );
        assert!(scene.objects()[&frame].emission > 5. && scene.objects()[&frame].opacity > 0.65);
        assert!(scene.objects()[&display.chain_rows[1].as_ref().unwrap().icons[0]].emission > 4.);
        display.advance(&mut scene, CHAIN_PULSE_SECONDS * 0.27);
        assert!(
            (scene.objects()[&last].transform.scale.x / (display.fruit_scale) - 0.75).abs() < 1e-5
        );
        assert!(
            (scene.objects()[&frame].transform.scale.x / (display.frame_scale.x) - 0.75).abs()
                < 1e-5
        );
        display.advance(&mut scene, CHAIN_PULSE_SECONDS);
        assert!(
            (scene.objects()[&frame].transform.scale.x / (display.frame_scale.x) - 1.).abs() < 1e-5
        );
        for row in display.chain_rows.iter().flatten() {
            for id in &row.icons {
                assert!((scene.objects()[id].transform.position.y - display.origin.y).abs() < 1e-6);
            }
        }
        assert_eq!(scene.objects()[&last].emission, 3.);
        assert_eq!(scene.objects()[&frame].emission, STOCK_FRAME_EMISSION);
        assert_eq!(scene.objects()[&frame].opacity, 1.);
        assert!((scene.objects()[&last].transform.scale.x - display.fruit_scale).abs() < 1e-6);
        display.sync_chains(&mut scene, &chains);
        assert_eq!(scene.objects()[&last].emission, 3.); // unchanged state does not retrigger
        display.clear(&mut scene);
        assert_eq!(display.chain_pulse, [0.; 2]);
        assert!(scene.variant_instances.is_empty());
    }
    #[test]
    fn combo_display_bounds_history_and_shared_instance_lifetime() {
        let (mut scene, mut display) = fixture();
        for _ in 0..8 {
            display.event(
                &mut scene,
                ComboEvent {
                    failed: None,
                    kind: "A".into(),
                    count: 3,
                    success: true,
                    points: 10,
                },
            );
        }
        assert_eq!(display.history.len(), 3);
        assert_eq!(scene.variant_instances.len(), 18);
        display.event(
            &mut scene,
            ComboEvent {
                failed: None,
                kind: "B".into(),
                count: 2,
                success: false,
                points: 0,
            },
        );
        assert_eq!(scene.variant_instances.len(), 23);
        assert_eq!(scene.mesh_variants.len(), 1);
        display.reset(&mut scene);
        assert!(scene.variant_instances.is_empty());
    }
}
