use cgmath::Vector3;
use strum_macros::EnumIter;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter)]
pub enum FaceDirection {
    Top,
    Bottom,
    West,
    East,
    North,
    South,
}

impl FaceDirection {
    #[allow(dead_code)]
    pub fn from_dir(dir: &Vector3<i32>) -> Option<Self> {
        if dir.x == 0 && dir.y == 1 && dir.z == 0 {
            Some(FaceDirection::Top)
        } else if dir.x == 0 && dir.y == -1 && dir.z == 0 {
            Some(FaceDirection::Bottom)
        } else if dir.x == -1 && dir.y == 0 && dir.z == 0 {
            Some(FaceDirection::West)
        } else if dir.x == 1 && dir.y == 0 && dir.z == 0 {
            Some(FaceDirection::East)
        } else if dir.x == 0 && dir.y == 0 && dir.z == -1 {
            Some(FaceDirection::North)
        } else if dir.x == 0 && dir.y == 0 && dir.z == 1 {
            Some(FaceDirection::South)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn as_dir(&self) -> Vector3<i32> {
        match self {
            FaceDirection::Top => Vector3::new(0, 1, 0),
            FaceDirection::Bottom => Vector3::new(0, -1, 0),
            FaceDirection::West => Vector3::new(-1, 0, 0),
            FaceDirection::East => Vector3::new(1, 0, 0),
            FaceDirection::North => Vector3::new(0, 0, -1),
            FaceDirection::South => Vector3::new(0, 0, 1),
        }
    }

    #[allow(dead_code)]
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(FaceDirection::Top),
            1 => Some(FaceDirection::Bottom),
            2 => Some(FaceDirection::West),
            3 => Some(FaceDirection::East),
            4 => Some(FaceDirection::North),
            5 => Some(FaceDirection::South),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_index(&self) -> usize {
        match self {
            FaceDirection::Top => 0,
            FaceDirection::Bottom => 1,
            FaceDirection::West => 2,
            FaceDirection::East => 3,
            FaceDirection::North => 4,
            FaceDirection::South => 5,
        }
    }

    pub fn brightness(&self) -> u8 {
        match self {
            FaceDirection::Top => 0,
            FaceDirection::Bottom => 3,
            FaceDirection::West => 1,
            FaceDirection::East => 1,
            FaceDirection::North => 2,
            FaceDirection::South => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter)]
pub enum SideDirection {
    Top,
    Bottom,
    Side,
}

impl From<SideDirection> for FaceDirection {
    fn from(val: SideDirection) -> Self {
        match val {
            SideDirection::Top => FaceDirection::Top,
            SideDirection::Bottom => FaceDirection::Bottom,
            SideDirection::Side => FaceDirection::South,
        }
    }
}

impl From<FaceDirection> for SideDirection {
    fn from(val: FaceDirection) -> Self {
        match val {
            FaceDirection::Top => SideDirection::Top,
            FaceDirection::Bottom => SideDirection::Bottom,
            FaceDirection::West | FaceDirection::East | FaceDirection::South | FaceDirection::North => {
                SideDirection::Side
            }
        }
    }
}
