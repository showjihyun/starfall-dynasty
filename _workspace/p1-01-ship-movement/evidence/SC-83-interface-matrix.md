# SC-83 경계면 비교표 (스키마 / Rust / C#)

- 판정: **PASS** — 신규 7타입 대조 행 **201개**, 불일치 **0건**
- 정수 행 51개 (Rust 무손실 51 / C# 무손실 37)
- 좁힘 행 4개 (통과 4)

#### 와이어 4타입 — envelope + payload — 행 70개

| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust | C# |
|---|---|---|---|---|---|
| SET_SHIP_CONTROL | `command_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SET_SHIP_CONTROL | `command_type` | type-override | None / req=True / null=False | `SetShipControlType` | `string` Req=Always |
| SET_SHIP_CONTROL | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SET_SHIP_CONTROL | `client_sent_at` | envelope | string / req=True / null=True | `Option<RealTime>` | `string` Req=AllowNull |
| SET_SHIP_CONTROL | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `SetShipControlPayload` | `SetShipControlPayload` Req=Always |
| SET_SHIP_CONTROL | `payload.input_seq` | payload | integer / req=True / null=False 1..4294967295 | `InputSeq` | `uint` Req=Always |
| SET_SHIP_CONTROL | `payload.thrust_x_milli` | payload | integer / req=True / null=False -1000..1000 | `ControlAxisMilli` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.thrust_y_milli` | payload | integer / req=True / null=False -1000..1000 | `ControlAxisMilli` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.thrust_z_milli` | payload | integer / req=True / null=False -1000..1000 | `ControlAxisMilli` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.roll_milli` | payload | integer / req=True / null=False -1000..1000 | `ControlAxisMilli` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.aim_x_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.aim_y_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.aim_z_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.aim_w_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SET_SHIP_CONTROL | `payload.brake` | payload | boolean / req=True / null=False | `bool` | `bool` Req=Always |
| SET_SHIP_CONTROL | `payload.flight_assist` | payload | boolean / req=True / null=False | `bool` | `bool` Req=Always |
| WORLD_SNAPSHOT | `message_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| WORLD_SNAPSHOT | `message_type` | type-override | None / req=True / null=False | `WorldSnapshotType` | `string` Req=Always |
| WORLD_SNAPSHOT | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| WORLD_SNAPSHOT | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| WORLD_SNAPSHOT | `correlation_id` | envelope | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| WORLD_SNAPSHOT | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `WorldSnapshotPayload` | `WorldSnapshotPayload` Req=Always |
| WORLD_SNAPSHOT | `payload.star_system_id` | payload | string / req=True / null=False | `DataId` | `string` Req=Always |
| WORLD_SNAPSHOT | `payload.soft_boundary_radius_mm` | payload | integer / req=True / null=False 1..20000000 | `i64` | `int` Req=Always |
| WORLD_SNAPSHOT | `payload.hard_boundary_radius_mm` | payload | integer / req=True / null=False 1..20000000 | `i64` | `int` Req=Always |
| WORLD_SNAPSHOT | `payload.snapshot_interval_ticks` | payload | integer / req=True / null=False 1..255 | `u16` | `int` Req=Always |
| WORLD_SNAPSHOT | `payload.controlled_ship_id` | payload | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| WORLD_SNAPSHOT | `payload.ack_input_seq` | payload | integer / req=True / null=True 1..4294967295 | `Option<InputSeq>` | `uint?` Req=AllowNull |
| WORLD_SNAPSHOT | `payload.ships` | payload | array / req=True / null=False | `Vec<ShipState>` | `ShipState[]` Req=Always |
| SHIP_SPAWNED | `event_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `event_type` | type-override | None / req=True / null=False | `ShipSpawnedType` | `string` Req=Always |
| SHIP_SPAWNED | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SHIP_SPAWNED | `world_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| SHIP_SPAWNED | `sequence` | envelope | None / req=True / null=False | `Sequence` | `long` Req=Always |
| SHIP_SPAWNED | `occurred_at` | envelope | string / req=True / null=False | `GameTime` | `string` Req=Always |
| SHIP_SPAWNED | `recorded_at` | envelope | string / req=True / null=False | `RealTime` | `string` Req=Always |
| SHIP_SPAWNED | `correlation_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `causation_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `actor_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `ShipSpawnedPayload` | `ShipSpawnedPayload` Req=Always |
| SHIP_SPAWNED | `payload.ship_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `payload.session_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_SPAWNED | `payload.ship_class_id` | payload | string / req=True / null=False | `DataId` | `string` Req=Always |
| SHIP_SPAWNED | `payload.star_system_id` | payload | string / req=True / null=False | `DataId` | `string` Req=Always |
| SHIP_SPAWNED | `payload.position_x_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| SHIP_SPAWNED | `payload.position_y_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| SHIP_SPAWNED | `payload.position_z_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| SHIP_SPAWNED | `payload.orientation_x_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SHIP_SPAWNED | `payload.orientation_y_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SHIP_SPAWNED | `payload.orientation_z_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SHIP_SPAWNED | `payload.orientation_w_micro` | payload | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| SHIP_DESPAWNED | `event_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `event_type` | type-override | None / req=True / null=False | `ShipDespawnedType` | `string` Req=Always |
| SHIP_DESPAWNED | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SHIP_DESPAWNED | `world_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| SHIP_DESPAWNED | `sequence` | envelope | None / req=True / null=False | `Sequence` | `long` Req=Always |
| SHIP_DESPAWNED | `occurred_at` | envelope | string / req=True / null=False | `GameTime` | `string` Req=Always |
| SHIP_DESPAWNED | `recorded_at` | envelope | string / req=True / null=False | `RealTime` | `string` Req=Always |
| SHIP_DESPAWNED | `correlation_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `causation_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `actor_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `ShipDespawnedPayload` | `ShipDespawnedPayload` Req=Always |
| SHIP_DESPAWNED | `payload.ship_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `payload.last_session_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SHIP_DESPAWNED | `payload.despawn_reason` | payload | string / req=True / null=False | `DespawnReason` | `string` Req=Always |
| SHIP_DESPAWNED | `payload.position_x_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| SHIP_DESPAWNED | `payload.position_y_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| SHIP_DESPAWNED | `payload.position_z_mm` | payload | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |

#### 배열 원소 ShipState (별도 표) — 행 23개

| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust | C# |
|---|---|---|---|---|---|
| WORLD_SNAPSHOT | `ShipState[].ship_id` | ShipState[] | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].actor_id` | ShipState[] | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].ship_class_id` | ShipState[] | string / req=True / null=False | `DataId` | `string` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].presence` | ShipState[] | string / req=True / null=False | `ShipPresence` | `string` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].position_x_mm` | ShipState[] | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].position_y_mm` | ShipState[] | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].position_z_mm` | ShipState[] | integer / req=True / null=False -1000000000000..1000000000000 | `PositionMm` | `long` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].velocity_x_mm_s` | ShipState[] | integer / req=True / null=False -100000000..100000000 | `VelocityMmPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].velocity_y_mm_s` | ShipState[] | integer / req=True / null=False -100000000..100000000 | `VelocityMmPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].velocity_z_mm_s` | ShipState[] | integer / req=True / null=False -100000000..100000000 | `VelocityMmPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].orientation_x_micro` | ShipState[] | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].orientation_y_micro` | ShipState[] | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].orientation_z_micro` | ShipState[] | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].orientation_w_micro` | ShipState[] | integer / req=True / null=False -1000000..1000000 | `QuaternionComponentMicro` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].angular_velocity_x_mdeg_s` | ShipState[] | integer / req=True / null=False -3600000..3600000 | `AngularVelocityMdegPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].angular_velocity_y_mdeg_s` | ShipState[] | integer / req=True / null=False -3600000..3600000 | `AngularVelocityMdegPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].angular_velocity_z_mdeg_s` | ShipState[] | integer / req=True / null=False -3600000..3600000 | `AngularVelocityMdegPerSecond` | `int` Req=Always |
| WORLD_SNAPSHOT | `ShipState[].angular_velocity_roll_mdeg_s` | ShipState[] | integer / req=True / null=False -3600000..3600000 | `AngularVelocityMdegPerSecond` | `int` Req=Always |
| STAR_SYSTEM | `ReferenceMarker[].id` | ReferenceMarker[] | string / req=True / null=False | `DataId` | **없음** |
| STAR_SYSTEM | `ReferenceMarker[].display_name` | ReferenceMarker[] | string / req=True / null=False | `String` | **없음** |
| STAR_SYSTEM | `ReferenceMarker[].kind` | ReferenceMarker[] | string / req=True / null=False | `ReferenceMarkerKind` | **없음** |
| STAR_SYSTEM | `ReferenceMarker[].position_m` | ReferenceMarker[] | array / req=True / null=False | `Triplet` | **없음** |
| STAR_SYSTEM | `ReferenceMarker[].visual_radius_m` | ReferenceMarker[] | number / req=True / null=False | `f64` | **없음** |

#### 데이터 3타입 — C# 열 없음이 정상 (AC-10 d) — 행 108개

| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust |
|---|---|---|---|---|
| SHIP_CLASS | `schema_version` | data | integer / req=True / null=False 1..2147483647 | `SchemaVersion` |
| SHIP_CLASS | `id` | data | string / req=True / null=False | `DataId` |
| SHIP_CLASS | `display_name` | data | string / req=True / null=False | `String` |
| SHIP_CLASS | `hull_class` | data | string / req=False / null=False | `Option<String>` |
| SHIP_CLASS | `gdd_source` | data | string / req=False / null=False | `Option<String>` |
| SHIP_CLASS | `designer_note` | data | string / req=False / null=False | `Option<String>` |
| SHIP_CLASS | `movement` | data | object / req=True / null=False | `ShipClassMovement` |
| SHIP_CLASS | `movement.model` | data.nested | string / req=True / null=False | `FlightModel` |
| SHIP_CLASS | `movement.max_speed_mps` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.main_thrust_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.reverse_thrust_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.lateral_thrust_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.brake_decel_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.assist_linear_decel_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.assist_lateral_decel_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| SHIP_CLASS | `movement.turn_rate_max_deg_s` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.turn_accel_deg_s2` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.turn_gain_deg_s_per_sin_half` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.turn_deadzone_sin_half` | data.nested | number / req=True / null=False 0..0.5 | `f64` |
| SHIP_CLASS | `movement.roll_rate_max_deg_s` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.roll_accel_deg_s2` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `movement.auto_level_rate_deg_s` | data.nested | number / req=True / null=False 0..3600 | `f64` |
| SHIP_CLASS | `movement.auto_level_deadzone_sin` | data.nested | number / req=True / null=False 0..0.5 | `f64` |
| SHIP_CLASS | `derived_for_review` | data | object / req=False / null=False | `Option<DerivedForReview>` |
| SHIP_CLASS | `derived_for_review.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SHIP_CLASS | `derived_for_review.zero_to_max_speed_s` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.zero_to_max_speed_distance_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.brake_from_max_speed_s` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.brake_from_max_speed_distance_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.coast_to_rest_from_max_speed_s` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.coast_to_rest_distance_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.flip_180_deg_s` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.position_delta_per_tick_at_max_speed_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `derived_for_review.rotation_delta_per_tick_at_max_rate_deg` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `geometry` | data | object / req=True / null=False | `ShipClassGeometry` |
| SHIP_CLASS | `geometry.hull_radius_m` | data.nested | number / req=True / null=False | `f64` |
| SHIP_CLASS | `geometry.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01` | data | object / req=False / null=False | `Option<ParkedStats>` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.crew` | data.nested | integer / req=False / null=False 0..100000 | `Option<i64>` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.cargo_capacity_t` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.weapon_hardpoints` | data.nested | integer / req=False / null=False 0..1000 | `Option<i64>` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.warp_range_ly` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| SHIP_CLASS | `reference_stats_unused_in_p1_01.shield` | data.nested | number / req=False / null=False 0..None | `Option<f64>` |
| STAR_SYSTEM | `schema_version` | data | integer / req=True / null=False 1..2147483647 | `SchemaVersion` |
| STAR_SYSTEM | `id` | data | string / req=True / null=False | `DataId` |
| STAR_SYSTEM | `display_name` | data | string / req=True / null=False | `String` |
| STAR_SYSTEM | `designer_note` | data | string / req=False / null=False | `Option<String>` |
| STAR_SYSTEM | `coordinate_space` | data | object / req=True / null=False | `CoordinateSpace` |
| STAR_SYSTEM | `coordinate_space.frame` | data.nested | string / req=True / null=False | `CoordinateFrame` |
| STAR_SYSTEM | `coordinate_space.unit` | data.nested | string / req=True / null=False | `CoordinateUnit` |
| STAR_SYSTEM | `coordinate_space.server_type` | data.nested | string / req=True / null=False | `ServerScalarType` |
| STAR_SYSTEM | `coordinate_space.origin` | data.nested | array / req=True / null=False | `Triplet` |
| STAR_SYSTEM | `coordinate_space.up_axis` | data.nested | array / req=True / null=False | `Triplet` |
| STAR_SYSTEM | `coordinate_space.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| STAR_SYSTEM | `play_area` | data | object / req=True / null=False | `PlayArea` |
| STAR_SYSTEM | `play_area.soft_boundary_radius_m` | data.nested | number / req=True / null=False | `f64` |
| STAR_SYSTEM | `play_area.hard_boundary_radius_m` | data.nested | number / req=True / null=False | `f64` |
| STAR_SYSTEM | `play_area.boundary_pull_mps2` | data.nested | number / req=True / null=False 0..1000000 | `f64` |
| STAR_SYSTEM | `play_area.client_float32_ulp_at_hard_boundary_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>  /*serde(default)*/` |
| STAR_SYSTEM | `play_area.client_float32_ulp_visible_jitter_threshold_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>  /*serde(default)*/` |
| STAR_SYSTEM | `play_area.floating_origin_required_above_radius_m` | data.nested | number / req=False / null=False 0..None | `Option<f64>  /*serde(default)*/` |
| STAR_SYSTEM | `play_area.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| STAR_SYSTEM | `spawn` | data | object / req=True / null=False | `Spawn` |
| STAR_SYSTEM | `spawn.ring_radius_m` | data.nested | number / req=False / null=False 0..20000 | `Option<f64>  /*serde(default)*/` |
| STAR_SYSTEM | `spawn.plane` | data.nested | string / req=False / null=False | `Option<SpawnPlane>  /*serde(default)*/` |
| STAR_SYSTEM | `spawn.point_count` | data.nested | integer / req=True / null=False 1..64 | `i64` |
| STAR_SYSTEM | `spawn.y_alternation_m` | data.nested | number / req=False / null=False | `Option<f64>  /*serde(default)*/` |
| STAR_SYSTEM | `spawn.clearance_m` | data.nested | number / req=True / null=False 0..20000 | `f64` |
| STAR_SYSTEM | `spawn.max_probe_attempts` | data.nested | integer / req=True / null=False 1..1024 | `i64` |
| STAR_SYSTEM | `spawn.radial_offset_step_m` | data.nested | number / req=True / null=False 0..20000 | `f64` |
| STAR_SYSTEM | `spawn.initial_velocity_mps` | data.nested | array / req=True / null=False | `Triplet` |
| STAR_SYSTEM | `spawn.initial_facing` | data.nested | string / req=True / null=False | `InitialFacing` |
| STAR_SYSTEM | `spawn.assignment_rule` | data.nested | string / req=True / null=False | `String` |
| STAR_SYSTEM | `spawn.points_m` | data.nested | array / req=True / null=False | `Vec<Triplet>` |
| STAR_SYSTEM | `spawn.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| STAR_SYSTEM | `presence` | data | object / req=True / null=False | `Presence` |
| STAR_SYSTEM | `presence.linger_seconds` | data.nested | number / req=True / null=False 0..3600 | `i64` |
| STAR_SYSTEM | `presence.reconnect_resume_window_seconds` | data.nested | number / req=True / null=False 0..3600 | `i64` |
| STAR_SYSTEM | `presence.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| STAR_SYSTEM | `reference_markers` | data | array / req=False / null=False | `Vec<ReferenceMarker>  /*serde(default)*/` |
| STAR_SYSTEM | `reference_markers_note` | data | string / req=False / null=False | `Option<String>` |
| SYNC_TUNING | `schema_version` | data | integer / req=True / null=False 1..2147483647 | `SchemaVersion` |
| SYNC_TUNING | `id` | data | string / req=True / null=False | `DataId` |
| SYNC_TUNING | `designer_note` | data | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SYNC_TUNING | `snapshot` | data | object / req=True / null=False | `SyncTuningSnapshot` |
| SYNC_TUNING | `snapshot.snapshot_hz` | data.nested | integer / req=True / null=False 1..60 | `i64` |
| SYNC_TUNING | `snapshot.max_entities_per_snapshot` | data.nested | integer / req=True / null=False 1..64 | `i64` |
| SYNC_TUNING | `snapshot.egress_budget_kib_s_per_session` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `snapshot.fallback_snapshot_hz_if_over_budget` | data.nested | integer / req=False / null=False 1..60 | `Option<i64>  /*serde(default)*/` |
| SYNC_TUNING | `snapshot.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SYNC_TUNING | `input` | data | object / req=True / null=False | `SyncTuningInput` |
| SYNC_TUNING | `input.client_send_hz` | data.nested | integer / req=True / null=False 1..240 | `i64` |
| SYNC_TUNING | `input.carry_forward_max_ticks` | data.nested | integer / req=True / null=False 0..200 | `i64` |
| SYNC_TUNING | `input.rate_limit_hz` | data.nested | integer / req=True / null=False 1..1000 | `i64` |
| SYNC_TUNING | `input.protocol_violation_hz` | data.nested | integer / req=True / null=False 1..10000 | `i64` |
| SYNC_TUNING | `input.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |
| SYNC_TUNING | `prediction` | data | object / req=True / null=False | `SyncTuningPrediction` |
| SYNC_TUNING | `prediction.remote_interp_delay_ms` | data.nested | number / req=True / null=False 0..2000 | `f64` |
| SYNC_TUNING | `prediction.remote_extrapolate_max_ms` | data.nested | number / req=True / null=False 0..5000 | `f64` |
| SYNC_TUNING | `prediction.reconcile_ignore_threshold_m` | data.nested | number / req=True / null=False 0..1 | `f64` |
| SYNC_TUNING | `prediction.reconcile_smooth_threshold_m` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `prediction.reconcile_smooth_duration_ms` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `prediction.reconcile_hard_snap_threshold_m` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `prediction.reconcile_orientation_ignore_threshold_deg` | data.nested | number / req=True / null=False 0..10 | `f64` |
| SYNC_TUNING | `prediction.reconcile_orientation_smooth_threshold_deg` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `prediction.reconcile_orientation_hard_snap_deg` | data.nested | number / req=True / null=False | `f64` |
| SYNC_TUNING | `prediction.note` | data.nested | string / req=False / null=False | `Option<String>  /*serde(default)*/` |

#### p0-02 4타입 (회귀) — 행 48개

| 타입 | 필드 | 출처 | 스키마(type / req / null / 범위) | Rust | C# |
|---|---|---|---|---|---|
| COMMAND_RESULT | `message_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| COMMAND_RESULT | `message_type` | type-override | None / req=True / null=False | `CommandResultType` | `string` Req=Always |
| COMMAND_RESULT | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| COMMAND_RESULT | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| COMMAND_RESULT | `correlation_id` | envelope | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| COMMAND_RESULT | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `CommandResultPayload` | `CommandResultPayload` Req=Always |
| COMMAND_RESULT | `payload.command_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| COMMAND_RESULT | `payload.status` | payload | string / req=True / null=False | `CommandStatus` | `string` Req=Always |
| COMMAND_RESULT | `payload.reason_code` | payload | string / req=True / null=True | `Option<RejectReasonCode>` | `string` Req=AllowNull |
| SESSION_READY | `message_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_READY | `message_type` | type-override | None / req=True / null=False | `SessionReadyType` | `string` Req=Always |
| SESSION_READY | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SESSION_READY | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| SESSION_READY | `correlation_id` | envelope | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| SESSION_READY | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `SessionReadyPayload` | `SessionReadyPayload` Req=Always |
| SESSION_READY | `payload.session_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_READY | `payload.world_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_READY | `payload.actor_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_READY | `payload.tick_hz` | payload | integer / req=True / null=False 1..1000 | `TickHz` | `int` Req=Always |
| SESSION_READY | `payload.server_version` | payload | string / req=True / null=False | `ServerVersion` | `string` Req=Always |
| SESSION_OPENED | `event_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_OPENED | `event_type` | type-override | None / req=True / null=False | `SessionOpenedType` | `string` Req=Always |
| SESSION_OPENED | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SESSION_OPENED | `world_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_OPENED | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| SESSION_OPENED | `sequence` | envelope | None / req=True / null=False | `Sequence` | `long` Req=Always |
| SESSION_OPENED | `occurred_at` | envelope | string / req=True / null=False | `GameTime` | `string` Req=Always |
| SESSION_OPENED | `recorded_at` | envelope | string / req=True / null=False | `RealTime` | `string` Req=Always |
| SESSION_OPENED | `correlation_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_OPENED | `causation_id` | envelope | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| SESSION_OPENED | `actor_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_OPENED | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `SessionOpenedPayload` | `SessionOpenedPayload` Req=Always |
| SESSION_OPENED | `payload.session_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_OPENED | `payload.transport` | payload | string / req=True / null=False | `SessionTransport` | `string` Req=Always |
| SESSION_CLOSED | `event_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_CLOSED | `event_type` | type-override | None / req=True / null=False | `SessionClosedType` | `string` Req=Always |
| SESSION_CLOSED | `schema_version` | type-override | None / req=True / null=False | `ConstSchemaVersion<1>` | `int` Req=Always |
| SESSION_CLOSED | `world_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_CLOSED | `tick` | envelope | None / req=True / null=False | `Tick` | `long` Req=Always |
| SESSION_CLOSED | `sequence` | envelope | None / req=True / null=False | `Sequence` | `long` Req=Always |
| SESSION_CLOSED | `occurred_at` | envelope | string / req=True / null=False | `GameTime` | `string` Req=Always |
| SESSION_CLOSED | `recorded_at` | envelope | string / req=True / null=False | `RealTime` | `string` Req=Always |
| SESSION_CLOSED | `correlation_id` | envelope | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_CLOSED | `causation_id` | envelope | string / req=True / null=True | `Option<UuidV7>` | `System.Guid?` Req=AllowNull |
| SESSION_CLOSED | `actor_id` | type-override | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_CLOSED | `payload` | envelope(payload 컨테이너) | object / req=True / null=False | `SessionClosedPayload` | `SessionClosedPayload` Req=Always |
| SESSION_CLOSED | `payload.session_id` | payload | string / req=True / null=False | `UuidV7` | `System.Guid` Req=Always |
| SESSION_CLOSED | `payload.close_reason` | payload | string / req=True / null=False | `SessionCloseReason` | `string` Req=Always |
