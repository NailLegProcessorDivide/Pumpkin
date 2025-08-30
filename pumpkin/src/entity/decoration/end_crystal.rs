use core::f32;
use std::sync::Arc;

use crate::entity::{Entity, EntityBase, NBTStorage, living::LivingEntity};

use pumpkin_data::damage::DamageType;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::{MetaDataType, Metadata};
use pumpkin_util::math::vector3::Vector3;

pub struct EndCrystalEntity {
    entity: Entity,
}

impl EndCrystalEntity {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

impl EndCrystalEntity {
    pub fn set_show_bottom(&self, show_bottom: bool) {
        self.entity
            .send_meta_data(&[Metadata::new(9, MetaDataType::Boolean, show_bottom)]);
    }
}


impl NBTStorage for EndCrystalEntity {
    fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn read_nbt_non_mut(&self, _nbt: &NbtCompound) {}
}


impl EntityBase for EndCrystalEntity {
    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn damage_with_context(
        &self,
        _caller: Arc<dyn EntityBase>,
        _amount: f32,
        damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        if damage_type != DamageType::EXPLOSION {
            self.entity.world.explode(self.entity.pos.load(), 6.0);
        }

        // TODO
        self.entity.remove();
        true
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }
}
