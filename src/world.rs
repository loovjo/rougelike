use std::u16;

use ext::*;
use controls::Action;
use block;
use entity;
use entity::{EntityWrapper, Player, Josef};
use shape::Shape;
use difficulty::Difficulty;
use inventory::InventoryItem;
use move_dir::{MoveDir, random_dir, DIRECTIONS};

use std::collections::HashMap;
use std::mem;
use std::sync::mpsc::Sender;

pub const HOTBAR_HEIGHT: u16 = 5;
pub const SCROLL_FOLLOW_DIST: i16 = 10;

#[derive(Debug)]
pub enum MetaAction {
    Die, Win
}

pub struct World {
    pub blocks: Vec<Vec<block::Block>>,
    pub entities: HashMap<u64, entity::EntityWrapper>,
    pub difficulty: Difficulty,
    pub auto: Vec<MoveDir>,
    action_sender: Sender<MetaAction>,
    pub scroll: (i16, i16),
}


impl World {
    pub fn empty(difficulty: Difficulty, action_sender: Sender<MetaAction>) -> World {
        World {
            blocks: vec![],
            entities: HashMap::new(),
            difficulty: difficulty,
            auto: vec![],
            action_sender: action_sender,
            scroll: (0, 0),
        }
    }

    pub fn tick(&mut self) {
        for k in self.entities.clone().keys() {
            if let Some(f) = self.entities.get(k).map(|x| x.get_tick_fn()) {
                f(self, *k);
            }
        }

        if !self.auto.is_empty() {
            if let Some(EntityWrapper::WPlayer(ref mut p)) =
                self.get_player_id().and_then(|id| self.entities.get_mut(&id))
            {
                let mut to_remove = None;
                if let Some((i, (InventoryItem::SuperBoots(ref mut d, max), ref mut count))) =
                    p.inventory.iter_mut()
                    .enumerate()
                    .find(|x| match (x.1).0 { InventoryItem::SuperBoots(_, _) => true, _ => false })
                {
                    *d -= 1;
                    if *d == 0 {
                        *count -= 1;
                        *d = *max;
                        if *count == 0 {
                            to_remove = Some(i);
                        }
                    }
                } else {
                    self.auto.clear();
                    return;
                }
                if let Some(to_remove) = to_remove {
                    p.inventory.remove(to_remove);
                }
            }

            let dir = self.auto.remove(0);
            self.get_player_id().map(|id| self.move_entity(id, &dir));
        }
    }

    pub fn update_scroll(&mut self, size: (u16, u16)) {
        if let Some(id) = self.get_player_id() {
            if let Some(en) = self.entities.get(&id) {
                if  self.scroll.0 > (en.get_pos().0 as i16) - SCROLL_FOLLOW_DIST {
                    self.scroll.0 = (en.get_pos().0 as i16) - SCROLL_FOLLOW_DIST;
                }
                if  self.scroll.1 > (en.get_pos().1 as i16) - SCROLL_FOLLOW_DIST {
                    self.scroll.1 = (en.get_pos().1 as i16) - SCROLL_FOLLOW_DIST;
                }
                if  self.scroll.0 < (en.get_pos().0 as i16) + SCROLL_FOLLOW_DIST - size.0 as i16 - 1 {
                    self.scroll.0 = (en.get_pos().0 as i16) + SCROLL_FOLLOW_DIST - size.0 as i16 - 1;
                }
                if  self.scroll.1 < (en.get_pos().1 as i16) + SCROLL_FOLLOW_DIST - (size.1 - 1 - HOTBAR_HEIGHT) as i16 {
                    self.scroll.1 = (en.get_pos().1 as i16) + SCROLL_FOLLOW_DIST - (size.1 - 1 - HOTBAR_HEIGHT) as i16;
                }
            }
        }
        if self.scroll.0 < 0 {
            self.scroll.0 = 0;
        }
        if self.scroll.1 < 0 {
            self.scroll.1 = 0;
        }
        if self.scroll.0 > self.blocks.len() as i16 - size.0 as i16 {
            self.scroll.0 = self.blocks.len() as i16 - size.0 as i16;
        }
        if self.scroll.1 > self.blocks[0].len() as i16 - size.1 as i16 + HOTBAR_HEIGHT as i16 {
            self.scroll.1 = self.blocks[0].len() as i16 - size.1 as i16 + HOTBAR_HEIGHT as i16;
        }
    }

    pub fn get_player_id(&self) -> Option<u64> {
        for (k, x) in &self.entities {
            if let &entity::EntityWrapper::WPlayer(_) = x {
                return Some(*k);
            }
        }
        None
    }

    pub fn do_metaaction(&mut self, action: MetaAction) {
        self.action_sender.send(action).expect("Can't send!");
    }

    pub fn do_action(&mut self, action: &Action) {
        match *action {
            Action::Move(dir) => {
                self.get_player_id().map(|id| self.move_entity(id, &dir));
                self.auto = vec![];
            }
            Action::Break(dir)  => {
                self.break_dir(dir);
                self.auto = vec![];
            }
            Action::Place(dir)  => {
                self.get_player_id() .map(|id| Player::place(self, dir, id));
                self.auto = vec![];
            }
            Action::Die => {
                self.do_metaaction(MetaAction::Die);
            }
            Action::IncActive => {
                self.get_player_id()
                    .and_then(|id| self.entities.get_mut(&id))
                    .map(|en| {
                        if let &mut EntityWrapper::WPlayer(ref mut pl) = en {
                            if pl.active < pl.inventory.len() - 1{
                                pl.active += 1;
                            }
                        }
                    });
            }
            Action::Run(dir) => {
                if let Some(EntityWrapper::WPlayer(p)) = self.get_player_id().and_then(|id| self.entities.get(&id)) {
                    let pos = p.pos;
                    let heur = |(x, y)| {
                        let score = match dir {
                            MoveDir::Left => pos.0.saturating_sub(x),
                            MoveDir::Right => x.saturating_sub(pos.0),
                            MoveDir::Up => pos.1.saturating_sub(y),
                            MoveDir::Down => y.saturating_sub(pos.1),
                        };
                        Some(score * 3)
                    };
                    self.auto = self.find_path(pos, heur, 1000).into_iter().take(20).collect();
                }
            }
            Action::DecActive => {
                self.get_player_id()
                    .and_then(|id| self.entities.get_mut(&id))
                    .map(|en| {
                        if let &mut EntityWrapper::WPlayer(ref mut pl) = en {
                            if pl.active > 0 {
                                pl.active -= 1;
                            }
                        }
                    });
            }
            _ => {}
        };

    }

    fn break_dir(&mut self, break_dir: MoveDir) {
        let new_pos;
        if let Some(player) = self.get_player_id().and_then(|id| self.entities.get(&id)) {
            let pl_pos = player.get_pos();
            let (dx, dy) = break_dir.to_vec();

            new_pos = (pl_pos.0 + dx as u16, pl_pos.1 + dy as u16);
        } else {
            return;
        }

        let block_pickup;
        if let Some(block_at) = self.blocks
            .get_mut(new_pos.0 as usize)
            .and_then(|x| x.get_mut(new_pos.1 as usize))
        {
            if block_at.is_breakable() {
                // Break block
                block_pickup = mem::replace(block_at, block::GROUND.clone());
            } else {
                return;
            }
        } else {
            return;
        }

        if let Some(&mut EntityWrapper::WPlayer(ref mut player)) =
            self.get_player_id().and_then(|id| self.entities.get_mut(&id))
        {
            player.pick_up(InventoryItem::Block(block_pickup.clone()));
        }

        self.get_player_id().map(|id| self.move_entity(id, &break_dir));
    }

    fn move_entity(&mut self, en_id: u64, move_dir: &MoveDir) -> bool {
        if let Some(en_move_fn) = self.entities.get(&en_id).map(|x| x.get_move_fn()) {
            en_move_fn(self, en_id, *move_dir)
        } else {
            false
        }

    }

    pub fn draw(&self, size: (u16, u16)) {

        // Draw world
        for x in 0..size.0 {
            for y in 0..size.1 - HOTBAR_HEIGHT {
                if let (Some(x_), Some(y_)) =
                    ((x as i16).checked_add(self.scroll.0), (y as i16).checked_add(self.scroll.1))
                {
                    if let Some(block) = self.blocks.get(x_ as usize)
                        .and_then(|col| col.get(y_ as usize))
                    {
                        block.get_shape().draw((x, y));
                    } else {
                        put_char((x as u16, y as u16), &Shape::empty());
                    }
                }
            }
        }

        // Clear hotbar
        for x in 0..size.0 {
            for y in size.1 - HOTBAR_HEIGHT..size.1 {
                put_char((x as u16, y as u16), &Shape::empty());
            }
        }

        // Draw entities
        self.entities.iter()
            .for_each(|(_, x)| x.pre_draw(self, &size, &self.scroll));

        self.entities.iter()
            .for_each(|(_, en)| {
                let (x, y) = en.get_pos();
                if let (Some(x_), Some(y_)) =
                    ((x as i16).checked_sub(self.scroll.0), (y as i16).checked_sub(self.scroll.1))
                {
                    if x_ >= 0 && x_ < size.0 as i16 && y_ >= 0 && y_ < size.1 as i16 - HOTBAR_HEIGHT as i16 {
                        en.get_shape().draw((x_ as u16, y_ as u16));
                    }
                }
            }
            );
    }

    pub fn generate(&mut self, width: usize, height: usize) {
        log("Generating!");

        self.blocks = vec![];

        for x in 0..width {
            self.blocks.push(vec![]);
            for _ in 0..height {
                if rand() > 0.1 {
                    self.blocks[x].push(block::WALL.clone());
                } else {
                    self.blocks[x].push(block::STONE.clone());
                }
            }
        }

        self.entities = HashMap::new();

        let mut placed = vec![];
        for _ in 0..10 * width * height {
            if rand() < 0.01 || placed.is_empty() {
                let x = (rand() * width as f64) as usize;
                let y = (rand() * height as f64) as usize;
                self.blocks[x][y] = block::GROUND.clone();
                placed.push((x, y, random_dir()));
            } else {
                let idx = (rand() * placed.len() as f64) as usize;
                let (x, y, mut dir) = placed[idx];

                if rand() < 0.05 {
                    dir = random_dir();
                }

                let dirv = dir.to_vec();

                let (nx, ny) = (x + dirv.0 as usize, y + dirv.1 as usize);

                let block_at = self.blocks.get(nx).and_then(|x| x.get(ny));
                if block_at == Some(&&*block::WALL) || block_at == Some(&&*block::WALL){
                    self.blocks[nx][ny] = block::GROUND.clone();
                    placed.push((nx, ny, dir));
                }
            }
        }

        let idx = (rand() * placed.len() as f64) as usize;
        let (x, y, _) = placed[idx];
        placed.remove(idx);
        self.add_entity(
            EntityWrapper::WPlayer(
                Player::new((x as u16, y as u16), self.difficulty.get_start_health())
                )
            );

        let idx = (rand() * placed.len() as f64) as usize;
        let (x, y, _) = placed[idx];
        placed.remove(idx);
        self.add_entity(
            EntityWrapper::WJosef(
                Josef::new(
                    (x as u16, y as u16),
                    self.difficulty.get_josef_police_rate(),
                    self.difficulty.get_josef_speed()
                    )
            ));

        log("Done!");
    }

    pub fn add_entity(&mut self, entity: EntityWrapper) {
        loop {
            let key = (rand() * <u64>::max_value() as f64) as u64;
            if !self.entities.contains_key(&key) {
                self.entities.insert(key, entity);
                break;
            }
        }
    }

    // Find a path using a heuristics function. If it returns None, it means that the best path is
    // found. Higher value in the heuristics function means closer to the goal.
    pub fn find_path(
        &self,
        from: (u16, u16),
        heuristics: impl Fn((u16, u16)) -> Option<u16>,
        steps: u16
        ) ->
        Vec<MoveDir>
    {

        let mut paths: Vec<(u16, Vec<MoveDir>, (u16, u16))> = vec![(0, vec![], from)];
        let mut best_path: Option<(u16, (Vec<MoveDir>, (u16, u16)))> = None;

        for _ in 0..steps {
            if paths.len() == 0 {
                break;
            }

            if let Some((_, from, pos)) = paths.pop() {
                for direction in &DIRECTIONS {
                    let new_pos = direction.move_vec(pos);

                    if paths.iter().any(|x| x.2 == new_pos) {
                        continue;
                    }

                    if let Some(block) = self.blocks
                        .get(new_pos.0 as usize)
                            .and_then(|x| x.get(new_pos.1 as usize))
                    {
                        if block.is_passable() {
                            let mut new_from = from.clone();
                            new_from.push(*direction);

                            // Check score

                            let score =
                                if let Some(score) = heuristics(new_pos) { score }
                                else { return new_from; };


                            let total_score = score.saturating_sub(new_from.len() as u16);

                            for i in 0..paths.len() + 1 {
                                if paths.get(i).map(|x| x.0).unwrap_or(u16::MAX) > total_score {
                                    paths.insert(i, (total_score, new_from.clone(), new_pos));
                                    break;
                                }
                            }


                            let best_score =
                                if let Some((best_score, _)) = best_path {
                                    best_score
                                } else {
                                    0
                                };

                            if total_score > best_score {
                                best_path = Some((total_score, (new_from, new_pos)));
                            }
                        }
                    }
                }
            }
        }

        best_path.map(|x| (x.1).0).unwrap_or(vec![])
    }
}
