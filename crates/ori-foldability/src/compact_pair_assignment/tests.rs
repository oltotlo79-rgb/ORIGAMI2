use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{
    FaceExtractionInput, LocalFlatFoldabilityReport, TopologySnapshot,
    analyze_local_flat_foldability, extract_faces_strict,
};
use serde::de::DeserializeOwned;

use super::*;

const REVISION: u64 = 41;

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
        .expect("fixed UUID fixture")
}

fn three_panel_accordion() -> (Paper, CreasePattern, TopologySnapshot) {
    let vertices = (0..8)
        .map(|index| fixed_id::<VertexId>(0x100 + index))
        .collect::<Vec<_>>();
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(4.0, 0.0),
        Point2::new(6.0, 0.0),
        Point2::new(6.0, 2.0),
        Point2::new(4.0, 2.0),
        Point2::new(2.0, 2.0),
        Point2::new(0.0, 2.0),
    ];
    let vertex_records = vertices
        .iter()
        .copied()
        .zip(positions)
        .map(|(id, position)| Vertex { id, position })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: fixed_id(0x200 + index as u64),
            start: vertices[index],
            end: vertices[(index + 1) % vertices.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fixed_id(0x301),
        start: vertices[1],
        end: vertices[6],
        kind: EdgeKind::Mountain,
    });
    edges.push(Edge {
        id: fixed_id(0x302),
        start: vertices[2],
        end: vertices[5],
        kind: EdgeKind::Valley,
    });
    let paper = Paper {
        boundary_vertices: vertices,
        ..Paper::default()
    };
    let pattern = CreasePattern {
        vertices: vertex_records,
        edges,
    };
    let topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("three-panel accordion topology");
    (paper, pattern, topology)
}

fn canonical_n33_compact_source_v2() -> (
    ProjectId,
    Paper,
    CreasePattern,
    TopologySnapshot,
    LocalFlatFoldabilityReport,
) {
    canonical_n33_compact_source_with_namespace_v2(canonical_n33_namespace_v2())
}

fn canonical_n33_namespace_v2() -> ProjectId {
    ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x4e, 0x5f, 0x56, 0x32, 0, 0, 2,
    ])
}

fn canonical_n33_compact_source_with_namespace_v2(
    namespace: ProjectId,
) -> (
    ProjectId,
    Paper,
    CreasePattern,
    TopologySnapshot,
    LocalFlatFoldabilityReport,
) {
    let cells = (0_i8..33)
        .flat_map(|index| {
            let x = index.checked_mul(2).expect("N33 fixture x fits i8");
            let y = if index % 2 == 0 { 0_i8 } else { -2_i8 };
            (x..=x + 2).flat_map(move |x| (y..=y + 2).map(move |y| (x, y)))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (pattern, paper) = compact_miura_pattern_v2(&cells, namespace);
    let topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("genuine N33 Miura topology");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    (namespace, paper, pattern, topology, local)
}

fn compact_miura_pattern_v2(cells: &[(i8, i8)], namespace: ProjectId) -> (CreasePattern, Paper) {
    let mut points = BTreeSet::new();
    let mut incidence = BTreeMap::<((i8, i8), (i8, i8)), (usize, (i8, i8), (i8, i8))>::new();
    for &(x, y) in cells {
        let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| entry.0 += 1)
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: VertexId::derive_v5(namespace, &[0xc1, (x + 4) as u8, (y + 4) as u8]),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i8, i8)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .expect("N33 cell corner")]
        .id
    };
    let edges = incidence
        .iter()
        .map(|(&(first, second), &(count, start, end))| Edge {
            id: EdgeId::derive_v5(
                namespace,
                &[
                    0xc2,
                    (first.0 + 4) as u8,
                    (first.1 + 4) as u8,
                    (second.0 + 4) as u8,
                    (second.1 + 4) as u8,
                ],
            ),
            start: vertex(start),
            end: vertex(end),
            kind: if count == 1 {
                EdgeKind::Boundary
            } else if first.1 == second.1 {
                EdgeKind::Mountain
            } else if first.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            },
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().expect("N33 boundary start");
        boundary.push(
            directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("next N33 boundary edge")
                .1,
        );
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    )
}

const N33_COMPACT_BITS_HEX_V2: &str = concat!(
    "321219000020040080800800410800501401000080844000c20c008000108000209fbf5fcfff7f6f33ffce3fef7297f9ffdf69ff65df73ef77efff3df6cfeaff",
    "ef7b0000000000000000000000000000000000000000000000000000000000000000c06ce4004288138981522220862140607f0400002012720f08b324202242",
    "116a84ccdfafe4ffbfb7997fa79b73b983ecffe7b4dd326fb1f79bf7ff1afb67f5fff76d04000000400000000110000010000020020000000980008010000000",
    "200000001000000000010000044000004000000000000000200002004000000000000000f8fffbfffffffffffffffffffff7ffffffffffffffffffffffffffff",
    "fffffffffff7fbfbffff6de6dffde75fff32ffff3bedbfed7beefffefdbfc7fe59fdffff9fc8000000250200a5440008428080b60800004024241e1066084004",
    "842004000100000000000000000000000000000000000000000000000000000000000000deafe4f7b9b7197aa79b73b9036cfee7a489226bb1f78137ff02fb62",
    "55bbd76de4000288138981522220842140607f0400002012720f083324202242116a840000000004000010000100000100002200000090000800880100000002",
    "0000801c4008712231504a06c4300408ec8f0000044442ee01619604444428428d103f92d2e34e62e0940e8e610a10d81f1100088c85dc03c23c0b8c8a50845e",
    "21bfffffffffe7ffff7ffefd3ffbfffff3ffdfffe7feffdfffffeeffffffffff0800001000004040040020040020880000004042200060060000000840000007",
    "10429c480c949201310c0102fb2300000191907b4098250111118a5023e4484a8f3b8981533a38862940607f4400203016720f08f32c302a42117a8504000008",
    "000020200200100200104500000020211000300300000004200000000040000000011100801000802002000000098100801900000020000100000400251202a5",
    "4440084280c0fe0800004024a41e10660840048422d400f9ffffffffffffffffffffffffffffffffffffffffffffffffffffffffff2fa93dee2d065ee9e458a6",
    "0089fd312180c858ec3d60ccb3c0a808c5e835fafdfff736f3effef32f7799ffff9df65ff63df77ffffedf63ffacfeffff2721c489c440291910c31420b03f02",
    "00101009b9078459121815a108bd42e6ffbfb7997fa79f77b98bfcffefb4ffb2efb9f7bbf7ff1efb67f5fff77dfeff7b9bf977fa7997bbc8ffff4efb2ffb1e7b",
    "bf7bffefb17f56ff7fdf13429c480c949201310c0102fb2300000191907b4098250111118a5023c4ff7f6f33ffce3fef7297f9ffdf69ff65df73ef77efff3df6",
    "cfeafffffbffffdf66feff7ffef53ff3ffbfd3fedbfee7feefdffffbec9fd5ffffff00a04442a0940808410810d81e0100008884c403c20c01888010841a20ed",
    "716f31f04a27c7320548ec8f090144c662ef01639e0546452846af517adc480c94d2c1314c0102fb2302008191907b4098658151118ad02b648f7b8b81573a39",
    "962940627f4c082032167b0f58f32c302b42317a8df6b8931838a59363980224f6c784002263a1f78031cf02a32214a15728c489c440291110c21020b03f0200",
    "001009b9078419121011a10835423ef73673ef74732e7780cdff9c3451642df63ef0e65f605fac6af7ba3d6e24064ae9a018a60081fd1101808048c83d20cc92",
    "c0a80845e815224e24064ae98018a60081fd1100808048c83d20cc92c0a80845e81512371203a574500c5380c0fe8800404024e41e10664960548422f40ac189",
    "c440291110c21020b03f0200001009b9078419021011a1083542b8b71978a59373b90264fec78480226bb1f78035df02fb6214a3d7ad7b9bb977ba39973bc8e6",
    "7f4e9b28b2167b1f78f32fb02f56bd7bdfdedbccbfd3cdb9dc41f6ff73da6699b7d8fbcdfb7f8dfdb3fafffb1600004000000000000000000000000002200000",
    "04000000000000804442a0940808610810d81f0100008884d403c20c01888050841a20040080840800410800501401000080844000c20c008000108000206f33",
    "ffee3fff7297f9ffdf69ff65df73fff7efff3df6cfeaffffff480c94d201314c0102fb2300000191907b4098258151118ad02b04000002220004210040510400",
    "0000120201083300000040000280901838a58363980204f6470400022321f78030cf02a32214a15788c4c0299d1cc31420b13f260410190bbd078c79161815a1",
    "08bd423ff7fffff3afffd9ffff9ffffffe3ff7fffffeff77ffffffffff070048890010840000451100000048481420cc0080080001080022065ee9e458a60089",
    "fd312180c858ec3d60ccb3c0a808c5e8353af3fffff3afff99ffff9df6dff63ff77ffffeff67ffacfeffff0740291100821020b03d0200001009890784190210",
    "01210835401878a59373990264fec784802263b1f78035df02fb2214a3d7ecfffffff9f7fffdffffefffffff9ffbffffffffffffffffffffffffffcfbfff67ff",
    "ff7ffefffbffdcfffffbffdffdffffffff3f504a0484300408ec8f0000004442ee01618600444428428d10a0940808410810d81e0100008884c403c20c018880",
    "10841a20feff7ffef53ffbfffff3ffdbffe7feefdfffffec9fdfffffffffff3ffffe9ffdfffff9ffefff73ffffefff7ff7ffffffff7fbdd3cdb9dc0136ff73d2",
    "4491b5d8fbc09b7f817db1aaddebb677ba39973bc0e67f4e9a28b2167b1f78f32fb02f56b57bdd7aa59373b90364fec7a481226bb1f78137ff02fb6215a3d7ed",
    "3bdd9ccb1d64fb3fa74d96598bbd4fbcff17d817abdebf6fa5440008420080b60800000024241e10660040048420040049e9e018a60081fd110180c048c83d20",
    "ccb2c0a80845e815020000000000000000000000000000008000000000000000a074722c5380c4fe981040642cf61e30e65960548422f40abdfffceb7fe6ff7f",
    "a7fdb7fdcffddfbffff7d83fabffffff2102401002001445000000202110803003002000042000483a39973b40e67f4c1a28b2167b1f78f32fb02f56317add12",
    "01200801008a22000000901008409801001000021000e43ffffa9ff9ffdf69ff6dff73fff7efff7df6cfeaffffff7ffee52ef3ffbfd3fecbbee7feefdfff7bec",
    "9fd5ffffff0800000800001001000080044000c00c000000100000003fef7217f9ffdf69ff65df73ef77efff3df6cfeaffeffb01314c0102fb2300000191907b",
    "4098258151118ad02b2420862140607f0400202012720f08b324202242116a8400000002000000000000000110000002000000000000c09ccb5d64ff3fa7fd96",
    "798fbddfbcfff7d83fabffbf6f732e7780ccff983451642df63ef0e65f605fac6af5bae518a60081fd112180c058c83d60ccb3c0a80845e815fadffff7ffffbf",
    "ffffff7feeffffffffffffffffffffffeffffbffffdfffffff3ff7ffffffffffffffffffff17c31420b03f0200101009b9078459121815a108bd4262980204f6",
    "470400022221f780304b02a32214a15708042140607b0400002012120f083304200242106a80b8dc45f6ff77da7fd9f7d8fbddfb7f8ffdb3fafffbf67297f9ff",
    "df69ff65df73ef77efff3df6cfeaffffff650a90d91f13028a8cc5de03d63c0bec8a508c5ea3610810d81f0100008884dc03c20c09888850841a21200400208a",
    "00000040422000610600000008400080ffefffff7fffffffffdcffffffffffffffffffff9f2940627f4c082032167b0f18f32c302a42117a85fecffefffffcff",
    "f7ffb9fffff7ffbffbffffffffff5de6ff7fa7fd977dcffddfbffff7d83fabffffff4380c0fe0800004024a41e10660840048422d40081000000110000004808",
    "0400cc00000000010000a00099ff3121a0c85aec3d60cdb7c0be08c5e8757b99ffff9df6dff63df77ffffedf63ffacfeffff1720b03f0200101009b907845912",
    "1815a108bd420264fec7a481226bb1f78137df02fb6214a3d72d0000000000000010000100200000000000000064feff77da7fdbf7dcfffdfb7f8ffdb3faffff",
    "1fd9ffcf69bb65de63ef37efff35f6cfeaffef5bfffffffbffffffe7feffdffffffefffffffffffffffffffffffffffffffffffffffffffffffff9ffdf69ff65",
    "df73ef77efff3df6cfeaffef7b36ff73da4491b5d8fbc09b7f817db1eaddfbf6ffffcfffffff9ffbff7ffffffbffffffffffffffefb4ffb6ffb9fffbf7ff3ffb",
    "67f5ffff3fa02d02000010098907841902100121080140ccff983451642df63ef0e65f605fac6af5bafdff3bedbfec7becfdeefdbfc7fe59fdff7d8ffd312180",
    "c058c83d60ccb3c0a80845e815cafe981050642cf61eb0e65960578462f41afd3fa74d96798bbddfbcffd7d83fabdfbf6fffe7b4c9226bb1f78977ff02fb62d5",
    "fbf76d7b0400002012120f083304200242102a808822000000901008409801000000021000e08f090145c662ef016b9e05f6452846af591001000080844000c0",
    "0c000000100000001401000080844401c20c008880108400208f0000004442e201618600444028420d9022000000909068409801001110821000440400000012",
    "0201003300000040000200440000002221f180304300222004210008020000000880000010000000000000009cf65fe63df67ef3fedf63ffacfeffbe3dedbfed",
    "7feefffefdffcffed9fdffff3f69a2c85aec7de0cdbfc0be58d5ea750b000000200002004000000000000000200410180bb9078c79161815a108bd42ffffffff",
    "fbffffffffffffffffffffffffffff9ffbff7ffffffbffffffffff03000191907b4098258151118ad02be4ffedff73fff7efff7ff6cfefffffff268aacc5de07",
    "defc0bec8b55ef5eb7068aacc5de07d67c0bec8b518c5eb7ffedff73fff7efff7ff6cfefffff7f0081b1907b4098678151118ad02b0428b2167b1f58f32db02f",
    "46317addde32efb1f79bf7ff1efb67f5fff76d96798bbddfbcffd7d83fabffbf6f51642df63ef0e65f605fac6af4bab5cc5becfde6fdbfc6fe59fdff7dbbcc7b",
    "ecfde6fdbfc7fe59fdff7d5b642df63ef0e65f605fac7af7be15198bbd07ac79169815a118bd46f7ffb9fffbf7ffbffbe7f7ffff3f2012720f08b32420224211",
    "6a84d8f7dcfffdfb7f8ffdb3faffffffff73ffffefff7ff7ffffffffffacc5de27defd0bec8b55efdfb78c85dc03c63c0b8c8a50845ea1ff73fff7efff7ff6cf",
    "effffffff7d8fbddfb7f8ffdb3fafffb3612720f08b32c302a42117a852021f1803043002220042100488bbdcfbcffd7d81fabdfbf6f2df61eb0e65b605f8462",
    "f4ba7deefffefdbfc7fe59fdffff4f000400800000000000000020e41e10e65960548422f40a89bddfbcffd7d83fabffbf6f011000000200000000000080fbff",
    "ffffffffffffffffffffffffffffffffffffffff7fef77efff3df6cfeaffeffbde03c63c0b8c8a50845ea14000c00c00000010000000ffffffffffffffffffff",
    "ff7bc098678151118ad02b1405083300200240000280f480304300222014a10688078419021001a10835407efffedf63ffacfeffff0700800000000000000030",
    "20cc0080080841080002106600000480000400018419001001210801408035df02fb6214a3d7ad79ffafb17f56bf7fdffefbffdffdffffffff1f6ffe05f6c5aa",
    "77ef5bdeff4bec8f55efdfb7deff7bec9fd5ffdfb7efff3df6cfeaffff7f980100000002100000f32c302a42117a8536df02fb6214a3d7ad79161815a118bd46",
    "ffffffffffffffff8700000000010000c000000000010000e05f605fac7af7befd17d817abdebf6f030000000400000000000000000000648151118ad02b8404",
    "444428428d100bec8b508c5eb30bec8b518c5eb7004440084200108151118ad02be4b37f56ffffff03222214a146c8d83fabffbf6f637fac7affbec5fe58f5fe",
    "7dcbfe59fdffffffffffffffffdf3fbfffffffa32214a146080240000280fcffffffff7f452846afd18a508c5ea30010800020452846af51118a50232456bd7f",
    "df2214a10608abdfbf6ffffffffffffdffff0f0108008262f4babdffffff15a3d76dffffff030000e0ffff7f841a20efdfb746afdbddeb160280a0d7e8bf6ff5",
    "baed750b00f0be054046088000e47b21b710d47e09",
);
const N33_PAIR_REGISTRY_SHA256_HEX_V2: &str =
    "d6b9e522cdb878fe53fd959cb41c8042e7ab29189c059e90bbff7594d6271935";

fn decode_compact_hex_v2(encoded: &str) -> Vec<u8> {
    assert!(encoded.len().is_multiple_of(2));
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let nibble = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("compact fixture must be lowercase hexadecimal"),
            };
            (nibble(pair[0]) << 4) | nibble(pair[1])
        })
        .collect()
}

#[test]
fn genuine_n33_compact_assignment_issues_without_search_v2() {
    let direction_bits = decode_compact_hex_v2(N33_COMPACT_BITS_HEX_V2);
    let digest_bytes = decode_compact_hex_v2(N33_PAIR_REGISTRY_SHA256_HEX_V2);
    let registry_digest: [u8; 32] = digest_bytes
        .try_into()
        .expect("N33 registry digest is exactly 32 bytes");
    assert_eq!(direction_bits.len(), 4_373);
    assert!(!facewise::compact_assignment_has_nonzero_tail_v2(
        &direction_bits,
        34_980,
    ));
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };

    let (namespace, paper, pattern, topology, local) = canonical_n33_compact_source_v2();
    let first = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
            source: GlobalFlatFoldabilityInput::current_with_geometry(
                namespace, &paper, &pattern, &topology, &local,
            ),
            variable_count: 34_980,
            variable_registry_sha256: registry_digest,
            direction_bits_le: &direction_bits,
        },
        limits,
    )
    .expect("fixed-namespace N33 compact assignment issues without search");
    assert_eq!(first.variable_count_v2(), 34_980);
    assert_eq!(first.variable_registry_sha256_v2(), registry_digest);
    assert_eq!(first.resources_v2().compact_assignment_bytes, 4_373);
    assert_eq!(first.work_counts_v2().search_nodes, 0);
    assert_eq!(first.layer_order_snapshot_v2().material_faces.len(), 265);
    assert_eq!(
        first.layer_order_snapshot_v2().face_pair_orders.len(),
        34_980
    );
    assert_eq!(
        first
            .layer_order_snapshot_v2()
            .proof_summary
            .expect("N33 facewise summary")
            .search_nodes,
        0,
    );
}

#[test]
fn compact_pair_assignment_reconstructs_without_search_and_is_exactly_bounded() {
    let (paper, pattern, topology) = three_panel_accordion();
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = || {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixed_id::<ProjectId>(1),
            &paper,
            &pattern,
            &topology,
            &local,
        )
    };
    let report = analyze_global_flat_foldability(source(), GlobalFlatFoldabilityLimits::default())
        .expect("baseline compact-assignment source");
    let snapshot = report.layer_order().expect("possible accordion source");
    let (variable_count, registry_digest, direction_bits) =
        facewise::compact_assignment_from_snapshot_for_test_v2(snapshot);
    assert_eq!(variable_count, 3);
    assert_eq!(direction_bits.len(), 1);

    let compact_input = || GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
        source: source(),
        variable_count,
        variable_registry_sha256: registry_digest,
        direction_bits_le: &direction_bits,
    };
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let baseline_limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };
    let authority = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        compact_input(),
        baseline_limits,
    )
    .expect("canonical complete assignment reconstructs without search");
    assert_eq!(authority.work_counts_v2().search_nodes, 0);
    assert_eq!(authority.variable_count_v2(), variable_count);
    assert_eq!(authority.variable_registry_sha256_v2(), registry_digest);
    assert_eq!(authority.exact_limits_v2(), baseline_limits);
    assert_eq!(
        authority
            .layer_order_snapshot_v2()
            .proof_summary
            .expect("facewise summary")
            .search_nodes,
        0
    );
    assert_eq!(
        authority.layer_order_snapshot_v2().face_pair_orders,
        snapshot.face_pair_orders
    );
    let resources = authority.resources_v2();
    assert_eq!(resources.compact_assignment_bytes, direction_bits.len());
    assert_eq!(
        resources.layer_order_retained_bytes,
        authority
            .layer_order_snapshot_v2()
            .checked_deep_retained_bytes_v1()
            .expect("issued retained bytes")
    );
    assert!(resources.observed_peak_bytes >= resources.borrowed_live_bytes);
    assert!(resources.observed_peak_bytes >= resources.layer_order_retained_bytes);

    let exact_limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        max_compact_assignment_bytes: resources.compact_assignment_bytes,
        max_layer_order_retained_bytes: resources.layer_order_retained_bytes,
        max_peak_bytes: resources.observed_peak_bytes,
        ..baseline_limits
    };
    issue_global_flat_layer_order_from_compact_pair_assignment_v2(compact_input(), exact_limits)
        .expect("all exact compact/result/peak equalities are admitted");
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_compact_assignment_bytes: resources.compact_assignment_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CompactPairAssignmentBytes,
                limit,
                observed,
            }
        }) if limit + 1 == observed && observed == resources.compact_assignment_bytes
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_layer_order_retained_bytes: resources.layer_order_retained_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::LayerOrderResultBytes,
                limit,
                observed,
            }
        }) if limit + 1 == observed && observed == resources.layer_order_retained_bytes
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_peak_bytes: resources.observed_peak_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limit,
                observed,
            }
        }) if limit + 1 == resources.observed_peak_bytes && observed > limit
    ));

    let internal_peak = resources
        .observed_facewise_peak_bytes
        .checked_sub(resources.borrowed_live_bytes)
        .expect("borrowed compact/canonical bytes are part of facewise peak");
    let certificate_limit = internal_peak.max(authority.work_counts_v2().certificate_bytes);
    let mut certificate_exact = exact_limits;
    certificate_exact.analysis.max_certificate_bytes = certificate_limit;
    issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        compact_input(),
        certificate_exact,
    )
    .expect("exact certificate/workspace equality is admitted");
    certificate_exact.analysis.max_certificate_bytes -= 1;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            certificate_exact,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        }) if limit + 1 == certificate_limit && observed > limit
    ));

    let mut pair_one_short = baseline_limits;
    pair_one_short.analysis.max_overlap_face_pairs = variable_count - 1;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            pair_one_short,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::OverlapFacePairs,
                limit,
                observed,
            }
        }) if limit + 1 == variable_count && observed == variable_count
    ));

    let retained = resources.layer_order_retained_bytes;
    authority
        .revalidate_live_source_v2(
            source(),
            GlobalFlatLayerOrderRevalidationLimitsV2 {
                analysis,
                max_source_retained_bytes: retained,
                max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
            },
        )
        .expect("consumer live revalidation rebuilds more than provenance");
}

#[test]
fn compact_pair_assignment_is_canonical_and_rejects_drift_tamper_and_stops() {
    struct StopObserver {
        remaining: usize,
        stop: GlobalFlatFoldabilityCheckpoint,
    }
    impl GlobalFlatFoldabilityObserver for StopObserver {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            if self.remaining == 0 {
                self.stop
            } else {
                self.remaining -= 1;
                GlobalFlatFoldabilityCheckpoint::Continue
            }
        }
    }

    let (paper, pattern, topology) = three_panel_accordion();
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = || {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixed_id::<ProjectId>(1),
            &paper,
            &pattern,
            &topology,
            &local,
        )
    };
    let report = analyze_global_flat_foldability(source(), GlobalFlatFoldabilityLimits::default())
        .expect("baseline canonical assignment");
    let snapshot = report.layer_order().expect("possible accordion source");
    let (variable_count, registry_digest, direction_bits) =
        facewise::compact_assignment_from_snapshot_for_test_v2(snapshot);
    let mut reordered_snapshot = snapshot.clone();
    reordered_snapshot.face_pair_orders.reverse();
    assert_eq!(
        facewise::compact_assignment_from_snapshot_for_test_v2(&reordered_snapshot),
        (variable_count, registry_digest, direction_bits.clone()),
        "registry and packed bits are independent of source record order"
    );

    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };
    let compact = |source, digest, bits: &[u8]| {
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source,
                variable_count,
                variable_registry_sha256: digest,
                direction_bits_le: bits,
            },
            limits,
        )
    };

    let mut bad_digest = registry_digest;
    bad_digest[0] ^= 1;
    assert!(matches!(
        compact(source(), bad_digest, &direction_bits),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count: variable_count + 1,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &direction_bits,
            },
            limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));
    let mut nonzero_tail = direction_bits.clone();
    *nonzero_tail.last_mut().unwrap() |= 0x80;
    assert!(matches!(
        compact(source(), registry_digest, &nonzero_tail),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::NonZeroTailBits
        ))
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &[],
            },
            limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::ByteLength {
                expected: 1,
                actual: 0,
            }
        ))
    ));
    let mut rejected_direction = false;
    for index in 0..variable_count {
        let mut tampered = direction_bits.clone();
        tampered[index / 8] ^= 1_u8 << (index % 8);
        if matches!(
            compact(source(), registry_digest, &tampered),
            Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::AssignmentRejected)
        ) {
            rejected_direction = true;
            break;
        }
    }
    assert!(
        rejected_direction,
        "at least one trusted hinge direction must reject bit tampering"
    );

    let mut spare_capacity_bits = Vec::with_capacity(direction_bits.len() + 257);
    spare_capacity_bits.extend_from_slice(&direction_bits);
    let spare_authority = compact(source(), registry_digest, &spare_capacity_bits)
        .expect("borrowed compact bytes are length-defined across allocators");
    assert_eq!(
        spare_authority.resources_v2().compact_assignment_bytes,
        direction_bits.len()
    );

    let paper_clone = paper.clone();
    let pattern_clone = pattern.clone();
    let topology_clone = topology.clone();
    let local_clone = local.clone();
    let equal_instance = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper_clone,
        &pattern_clone,
        &topology_clone,
        &local_clone,
    );
    let equal_authority = compact(equal_instance, registry_digest, &direction_bits)
        .expect("equal geometry in a separate allocation has the same registry bytes");
    assert_eq!(
        equal_authority.variable_registry_sha256_v2(),
        registry_digest
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: REVISION,
        paper: &paper,
        pattern: &reordered_pattern,
    })
    .expect("reordered semantic-equal topology");
    let reordered_local = analyze_local_flat_foldability(&paper, &reordered_pattern);
    let reordered_source = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper,
        &reordered_pattern,
        &reordered_topology,
        &reordered_local,
    );
    let reordered_authority = compact(reordered_source, registry_digest, &direction_bits)
        .expect("canonical registry ignores live record storage order");
    assert_eq!(
        reordered_authority.variable_registry_sha256_v2(),
        registry_digest
    );
    assert_eq!(
        reordered_authority
            .layer_order_snapshot_v2()
            .face_pair_orders,
        spare_authority.layer_order_snapshot_v2().face_pair_orders
    );

    let foreign_namespace = fixed_id::<ProjectId>(2);
    let foreign_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: foreign_namespace,
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("foreign-namespace topology");
    let foreign_source = GlobalFlatFoldabilityInput::current_with_geometry(
        foreign_namespace,
        &paper,
        &pattern,
        &foreign_topology,
        &local,
    );
    assert!(matches!(
        compact(foreign_source, registry_digest, &direction_bits),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));

    let mut drifted_pattern = pattern.clone();
    let coordinate = &mut drifted_pattern.vertices[0].position.x;
    *coordinate = f64::from_bits(coordinate.to_bits() + 1);
    let drifted_source = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper,
        &drifted_pattern,
        &topology,
        &local,
    );
    let drifted_authority = compact(drifted_source, registry_digest, &direction_bits)
        .expect("a still-valid assignment may be newly issued for changed geometry");
    assert_ne!(
        drifted_authority.provenance_v2().source_fingerprint,
        spare_authority.provenance_v2().source_fingerprint
    );
    assert!(
        spare_authority
            .revalidate_live_source_v2(
                drifted_source,
                GlobalFlatLayerOrderRevalidationLimitsV2 {
                    analysis,
                    max_source_retained_bytes: spare_authority
                        .resources_v2()
                        .layer_order_retained_bytes,
                    max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
                },
            )
            .is_err(),
        "one-ULP source drift invalidates the previously issued authority"
    );

    let mut invalid_limits = limits;
    invalid_limits.max_peak_bytes = usize::MAX;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &direction_bits,
            },
            invalid_limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::InvalidLimits)
    ));

    let observer_input = || GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
        source: source(),
        variable_count,
        variable_registry_sha256: registry_digest,
        direction_bits_le: &direction_bits,
    };
    let mut cancelled = StopObserver {
        remaining: 10,
        stop: GlobalFlatFoldabilityCheckpoint::Cancelled,
    };
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
            observer_input(),
            limits,
            &mut cancelled,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    let mut deadline = StopObserver {
        remaining: 10,
        stop: GlobalFlatFoldabilityCheckpoint::DeadlineReached,
    };
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
            observer_input(),
            limits,
            &mut deadline,
        ),
        Err(
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
            }
        )
    ));
}
