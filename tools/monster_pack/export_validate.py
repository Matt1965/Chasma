"""Validation for Option B FBX2glTF exports."""

from __future__ import annotations

import math
import struct
from pathlib import Path
from typing import Any

from glb_sanitize_animations import load_glb


def _acc(gltf, blob, i, fmt):
    a = gltf["accessors"][i]
    bv = gltf["bufferViews"][a["bufferView"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    size = struct.calcsize(fmt)
    stride = bv.get("byteStride", size)
    return [struct.unpack_from(fmt, blob, off + k * stride) for k in range(a["count"])]


def _qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def _qrot(q, v):
    x, y, z, w = q
    vx, vy, vz = v
    cx = y * vz - z * vy
    cy = z * vx - x * vz
    cz = x * vy - y * vx
    return (
        vx + 2 * (w * cx + y * cz - z * cy),
        vy + 2 * (w * cy + z * cx - x * cz),
        vz + 2 * (w * cz + x * cy - y * cx),
    )


def rest_world(gltf):
    parent = {}
    for i, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []):
            parent[c] = i
    cache = {}

    def wm(i):
        if i in cache:
            return cache[i]
        n = gltf["nodes"][i]
        t = tuple(n.get("translation", [0, 0, 0]))
        r = tuple(n.get("rotation", [0, 0, 0, 1]))
        s = tuple(n.get("scale", [1, 1, 1]))
        if i in parent:
            pt, pr, ps = wm(parent[i])
            wt = tuple(pt[k] + c for k, c in enumerate(_qrot(pr, tuple(t[k] * ps[k] for k in range(3)))))
            cache[i] = (wt, _qmul(pr, r), tuple(ps[k] * s[k] for k in range(3)))
        else:
            cache[i] = (t, r, s)
        return cache[i]

    return {i: wm(i) for i in range(len(gltf["nodes"]))}


def translation_scale_diagnostic(gltf, blob, skip_rm: bool = True) -> dict[str, Any]:
    joints = set()
    for s in gltf.get("skins", []):
        joints.update(s["joints"])
    wp = rest_world(gltf)
    pts = [wp[j][0] for j in joints]
    ext = max(max(p[k] for p in pts) - min(p[k] for p in pts) for k in range(3))
    worst = (0.0, None, None)
    for a in gltf.get("animations", []):
        name = a.get("name") or ""
        if skip_rm and name.endswith("_RM"):
            continue
        for ch in a["channels"]:
            tgt = ch["target"]
            if tgt["path"] != "translation" or tgt.get("node") not in joints:
                continue
            vals = _acc(gltf, blob, a["samplers"][ch["sampler"]]["output"], "<fff")
            pp = max(max(v[k] for v in vals) - min(v[k] for v in vals) for k in range(3))
            if pp > worst[0]:
                worst = (pp, name, gltf["nodes"][tgt["node"]].get("name"))
    ratio = worst[0] / ext if ext else float("inf")
    return {
        "skeleton_extent_m": ext,
        "worst_translation_pp_m": worst[0],
        "ratio": ratio,
        "clip": worst[1],
        "joint": worst[2],
        "red_flag": ratio > 1.0,
    }


def orientation_diagnostic(gltf) -> dict[str, Any]:
    names = {n.get("name"): i for i, n in enumerate(gltf["nodes"])}
    wp = rest_world(gltf)
    spine = None
    for pelvis, head in (("Bufomorph_ Pelvis", "Bufomorph_ Head"), ("Muscomorph_Body", "Muscomorph_Head")):
        if pelvis in names and head in names:
            a, b = wp[names[pelvis]][0], wp[names[head]][0]
            v = tuple(b[k] - a[k] for k in range(3))
            ln = math.sqrt(sum(c * c for c in v)) or 1.0
            spine = tuple(c / ln for c in v)
            break
    if spine is None:
        return {"spine_axis": None, "upright_ok": None}
    up_dot = spine[1]
    return {"spine_axis": spine, "up_dot_y": up_dot, "upright_ok": up_dot > 0.85}


def clip_completeness(expected: list[str], gltf) -> dict[str, Any]:
    have = {a.get("name") for a in gltf.get("animations", [])}
    missing = sorted(set(expected) - have)
    extra = sorted(have - set(expected))
    return {"expected_count": len(expected), "have_count": len(have), "missing": missing, "extra": extra}


def sample_channel_at(gltf, blob, anim_name: str, node_name: str, path: str, t: float):
    names = {n.get("name"): i for i, n in enumerate(gltf["nodes"])}
    if node_name not in names:
        return None
    ni = names[node_name]
    anim = next((a for a in gltf["animations"] if a.get("name") == anim_name), None)
    if not anim:
        return None
    for ch in anim["channels"]:
        if ch["target"].get("node") != ni or ch["target"]["path"] != path:
            continue
        s = anim["samplers"][ch["sampler"]]
        times = [v[0] for v in _acc(gltf, blob, s["input"], "<f")]
        fmt = "<fff" if path != "rotation" else "<ffff"
        vals = _acc(gltf, blob, s["output"], fmt)
        k = min(range(len(times)), key=lambda j: abs(times[j] - t))
        return vals[k], times[k]
    return None


def compare_merged_to_clip_source(
    merged_path: Path,
    clip_glbs: dict[str, Path],
    samples: list[tuple[str, str, str, float]],
) -> list[dict[str, Any]]:
    mg, mb = load_glb(merged_path)
    rows = []
    for clip, node, path, t in samples:
        merged = sample_channel_at(mg, mb, clip, node, path, t)
        cg, cb = load_glb(clip_glbs[clip])
        src_anim = cg["animations"][0]["name"]
        src = sample_channel_at(cg, cb, src_anim, node, path, t)
        if merged is None or src is None:
            rows.append({"clip": clip, "node": node, "path": path, "error": "missing channel"})
            continue
        mv, _ = merged
        sv, _ = src
        delta = max(abs(mv[i] - sv[i]) for i in range(len(mv)))
        rows.append({"clip": clip, "node": node, "path": path, "t": t, "delta": delta, "ok": delta < 1e-4})
    return rows


def validate_export(
    glb_path: Path,
    expected_clips: list[str],
    clip_glbs: dict[str, Path] | None = None,
) -> dict[str, Any]:
    gltf, blob = load_glb(glb_path)
    report = {
        "glb": str(glb_path),
        "translation_diagnostic": translation_scale_diagnostic(gltf, blob),
        "orientation": orientation_diagnostic(gltf),
        "clips": clip_completeness(expected_clips, gltf),
    }
    errors = []
    if report["translation_diagnostic"]["red_flag"]:
        errors.append(
            f"translation ratio {report['translation_diagnostic']['ratio']:.1f} > 1.0 "
            f"({report['translation_diagnostic']['clip']} / {report['translation_diagnostic']['joint']})"
        )
    if report["orientation"]["upright_ok"] is False:
        errors.append(f"rest spine axis not upright: {report['orientation']['spine_axis']}")
    if report["clips"]["missing"]:
        errors.append(f"missing clips: {report['clips']['missing']}")
    if clip_glbs:
        samples = [
            ("IdleLookAround", "Bufomorph_", "translation", 0.35),
            ("BiteAttack", "Bufomorph_", "translation", 0.35),
            ("IdleLookAround", "Bufomorph_JawUpper", "rotation", 0.0),
        ]
        if not any(n in {x.get("name") for x in gltf["nodes"]} for n in ("Bufomorph_",)):
            samples = [("BiteAttack", "Muscomorph_Body", "translation", 0.35)]
        report["source_agreement"] = compare_merged_to_clip_source(glb_path, clip_glbs, samples)
        for row in report["source_agreement"]:
            if row.get("ok") is False:
                errors.append(f"source mismatch {row}")
    report["complete"] = len(errors) == 0
    report["errors"] = errors
    return report
