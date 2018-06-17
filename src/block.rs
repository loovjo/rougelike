use std::mem::replace;
use level::Level;
use shape::Shape;
use ext::*;
use entity::*;

use std::sync::Mutex;

lazy_static! {
    pub static ref BLOCK_FUNCS: Mutex<Vec<fn(&mut Level, u64)>>
        = Mutex::new(vec![|_, _| {}]);
}

#[derive(Debug, Clone, Hash)]
pub struct Block {
    pub name: String,
    pub desc: String,
    pub shape: Shape,
    pub id: usize,
    pub passable: bool,
    pub breakable: bool
}

impl PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        self.id == other.id
    }
}
impl Eq for Block {}

impl Block {
    fn new(shape: Shape, name: String, desc: String, passable: bool, breakable: bool, on_walk: fn(&mut Level, u64))
        -> Block
    {
        let mut blkf = BLOCK_FUNCS.lock().unwrap();
        blkf.push(on_walk);

        Block {
            id: blkf.len() - 1,
            name: name,
            desc: desc,
            passable: passable,
            breakable: breakable,
            shape: shape
        }
    }

    #[inline]
    pub fn get_id(&self) -> usize { self.id }

    #[inline]
    pub fn get_shape(&self) -> Shape { self.shape }

    #[inline]
    pub fn is_passable(&self) -> bool { self.passable }

    #[inline]
    pub fn is_breakable(&self) -> bool { self.breakable }
}

lazy_static! {
    pub static ref GROUND: Block = Block::new(
        Shape::new('.', (128, 128, 128), (0, 0, 0)),
        "Ground".into(),
        "Passive ground".into(),
        true,
        false,
        |_, _| {}
        );

    pub static ref WALL: Block = Block::new(
        Shape::new('#', (202, 195, 210), (0, 0, 0)),
        "Wall".into(),
        "An wall".into(),
        false,
        true,
        |_, _| {}
        );

    pub static ref STONE: Block = Block::new(
        Shape::new('&', (120, 140, 160), (10, 30, 50)),
        "Stone".into(),
        "A stone".into(),
        false,
        true,
        |_, _| {}
        );

    pub static ref MOVER: Block = Block::new(
        Shape::new('^', (255, 240, 30), (0, 0, 0)),
        "Mover".into(),
        "Moves anything that walks on it randomly to somewhere on the map".into(),
        true,
        true,
        |level, id| {
            let pos;
            loop {
                let x = (rand() * level.blocks.len() as f64) as usize;
                let y = (rand() * level.blocks[0].len() as f64) as usize;

                let passable = level.blocks.get(x as usize)
                    .and_then(|a| a.get(y as usize))
                    .map(|a| a.is_passable())
                    .unwrap_or(false);

                if passable {
                    pos = (x as u16, y as u16);
                    break;
                }
            }
            if let Some(en) = level.entities.get_mut(&id) {
                let epos = en.get_pos_mut();
                epos.0 = pos.0;
                epos.1 = pos.1;
            }
        }
        );

    pub static ref STAIRS: Block = Block::new(
        Shape::new('>', (255, 240, 30), (0, 0, 0)),
        "Stairs".into(),
        "Moves you to the next/previous floor".into(),
        true,
        true,
        |level, id| {
            if let Some(EntityWrapper::WPlayer(_)) = level.entities.get(&id) {
                level.send_callback(Box::new(
                    |ref mut world| {
                        let next_active = world.other_levels.remove(0);
                        let old_active = replace(&mut world.active_level, next_active);
                        world.other_levels.push(old_active);
                    })).expect("Can't send!");
            }
        }
        );

    pub static ref COMMUNISM: Block = Block::new(
        Shape::new('☭', (253, 233, 54), (0, 0, 0)),
        "COMMUNISM".into(),
        "Heals you".into(),
        true,
        true,
        |level, id| {
            if let Some(EntityWrapper::WPlayer(player)) = level.entities.get_mut(&id) {
                player.hunger += 1;
                level.blocks[player.pos.0 as usize][player.pos.1 as usize] = GROUND.clone();
            }
        }
        );

}
