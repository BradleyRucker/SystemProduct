#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Discrete event simulation engine sidecar for SystemProduct.

Protocol
--------
stdin:  one JSON line:
        {
          "project_json": { "nodes": [...], "edges": [...], ... },
          "scenario": { "id", "name", "duration_ms", "events": [...] },
          "block_behaviors": { "<block_id>": { "sim_params": {...}, "sim_script": "..." } }
        }

stdout: one JSON line:
        {
          "status": "complete" | "error",
          "metrics": { "<block_id>": { "utilization", "queue_depth", "processed_count", "failures" } },
          "timeline": [ { "timestamp_ms", "block_id", "event_type", "detail" } ],
          "errors": [ ... ]
        }

stderr: diagnostic log messages only (never parsed by caller)
"""

from __future__ import annotations

import json
import logging
import sys
import traceback
from typing import Any, Dict, List, Optional

# ── UTF-8 everywhere ──────────────────────────────────────────────────────────
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stdin, "reconfigure"):
    sys.stdin.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

logging.basicConfig(
    stream=sys.stderr,
    level=logging.INFO,
    format="[simulation_engine] %(levelname)s %(message)s",
)

# ── Optional SimPy ────────────────────────────────────────────────────────────
try:
    import simpy
    SIMPY_OK = True
    logging.info("SimPy %s loaded", simpy.__version__)
except ImportError:
    simpy = None  # type: ignore
    SIMPY_OK = False
    logging.warning(
        "SimPy not installed — falling back to deterministic approximation. "
        "Install with: pip install simpy"
    )


# ── SimPy discrete event simulation ──────────────────────────────────────────

def run_with_simpy(
    project_json: Dict,
    scenario: Dict,
    block_behaviors: Dict,
) -> Dict:
    """Full discrete event simulation using SimPy."""
    import simpy as sp
    import random

    duration_ms: float = float(scenario.get("duration_ms") or 10000)
    timeline: List[Dict] = []

    nodes: List[Dict] = project_json.get("nodes") or []
    edges: List[Dict] = project_json.get("edges") or []

    block_nodes = {n["id"]: n for n in nodes if n.get("kind") == "block"}

    env = sp.Environment()

    # One SimPy Store per block (acts as input queue)
    block_stores: Dict[str, Any] = {}
    for block_id, _ in block_nodes.items():
        behavior = block_behaviors.get(block_id) or {}
        params = behavior.get("sim_params") or {}
        capacity = int(params.get("queue_capacity") or 100)
        block_stores[block_id] = sp.Store(env, capacity=capacity)

    # Per-block accumulators
    block_counters: Dict[str, Dict] = {
        bid: {"processed": 0, "failures": 0, "busy_time": 0.0}
        for bid in block_nodes
    }

    def block_process(env: Any, block_id: str, store: Any) -> Any:
        behavior = block_behaviors.get(block_id) or {}
        params = behavior.get("sim_params") or {}
        script: Optional[str] = behavior.get("sim_script")

        processing_time_ms = float(params.get("processing_time_ms") or 100.0)
        failure_rate = float(params.get("failure_rate") or 0.0)

        item_seq = 0
        while True:
            try:
                item = yield store.get()
                item_seq += 1
                start = env.now

                if script:
                    # Execute user-provided script in a sandboxed local context
                    local_ctx: Dict = {
                        "item": item,
                        "params": params,
                        "env_now": env.now,
                        "result": {
                            "failed": False,
                            "processing_time_ms": processing_time_ms,
                        },
                    }
                    try:
                        exec(script, {}, local_ctx)  # noqa: S102
                        failed = bool(local_ctx["result"].get("failed", False))
                        proc_time = float(
                            local_ctx["result"].get("processing_time_ms", processing_time_ms)
                        )
                    except Exception as exc:
                        logging.warning("Block %s script error: %s", block_id, exc)
                        failed = False
                        proc_time = processing_time_ms
                else:
                    failed = random.random() < failure_rate
                    proc_time = processing_time_ms

                yield env.timeout(proc_time)
                elapsed = env.now - start
                block_counters[block_id]["busy_time"] += elapsed

                if failed:
                    block_counters[block_id]["failures"] += 1
                    timeline.append(
                        {
                            "timestamp_ms": env.now,
                            "block_id": block_id,
                            "event_type": "failure",
                            "detail": f"item {item_seq} failed after {elapsed:.1f}ms",
                        }
                    )
                else:
                    block_counters[block_id]["processed"] += 1
                    timeline.append(
                        {
                            "timestamp_ms": env.now,
                            "block_id": block_id,
                            "event_type": "processed",
                            "detail": f"item {item_seq} done in {elapsed:.1f}ms",
                        }
                    )
                    # Forward output to downstream blocks via 'connects' edges
                    for edge in edges:
                        if (
                            edge.get("kind") == "connects"
                            and edge.get("source_id") == block_id
                            and edge.get("target_id") in block_stores
                        ):
                            target_store = block_stores[edge["target_id"]]
                            if len(target_store.items) < target_store.capacity:
                                target_store.put(item_seq)

            except sp.Interrupt:
                break

    def event_injector(env: Any) -> Any:
        events = sorted(scenario.get("events") or [], key=lambda e: float(e.get("time_ms") or 0))
        for ev in events:
            target_time = float(ev.get("time_ms") or 0)
            delay = max(0.0, target_time - env.now)
            yield env.timeout(delay)
            target_id = ev.get("block_id") or ev.get("target_id")
            if target_id and target_id in block_stores:
                store = block_stores[target_id]
                if len(store.items) < store.capacity:
                    store.put({"signal": ev.get("signal_type", "trigger"), "value": ev.get("value")})
                    timeline.append(
                        {
                            "timestamp_ms": env.now,
                            "block_id": target_id,
                            "event_type": "stimulus",
                            "detail": f"signal={ev.get('signal_type', 'trigger')}",
                        }
                    )

    # Launch all block processes
    for bid, store in block_stores.items():
        env.process(block_process(env, bid, store))

    # Launch stimulus injector
    scenario_events = scenario.get("events") or []
    if scenario_events:
        env.process(event_injector(env))

    env.run(until=duration_ms)

    # Build per-block metrics
    metrics: Dict[str, Dict] = {}
    for block_id in block_nodes:
        c = block_counters[block_id]
        util = c["busy_time"] / duration_ms if duration_ms > 0 else 0.0
        metrics[block_id] = {
            "utilization": round(min(util, 1.0), 4),
            "queue_depth": len(block_stores[block_id].items),
            "processed_count": c["processed"],
            "failures": c["failures"],
        }

    return {"status": "complete", "metrics": metrics, "timeline": timeline, "errors": []}


# ── Deterministic fallback (no SimPy) ────────────────────────────────────────

def run_deterministic(
    project_json: Dict,
    scenario: Dict,
    block_behaviors: Dict,
) -> Dict:
    """Theoretical approximation when SimPy is not installed."""
    nodes: List[Dict] = project_json.get("nodes") or []
    duration_ms = float(scenario.get("duration_ms") or 10000)
    metrics: Dict[str, Dict] = {}
    errors: List[str] = [
        "SimPy is not installed — results are theoretical approximations only. "
        "Install with: pip install simpy"
    ]

    for node in nodes:
        if node.get("kind") != "block":
            continue
        block_id = node["id"]
        behavior = block_behaviors.get(block_id) or {}
        params = behavior.get("sim_params") or {}

        processing_time_ms = float(params.get("processing_time_ms") or 100.0)
        failure_rate = float(params.get("failure_rate") or 0.0)
        throughput_per_sec = params.get("throughput_per_sec")

        if throughput_per_sec:
            processed = int(float(throughput_per_sec) * (duration_ms / 1000.0))
        elif processing_time_ms > 0:
            processed = int(duration_ms / processing_time_ms)
        else:
            processed = 0

        failures = int(processed * failure_rate)
        util = (
            min(processing_time_ms * processed / duration_ms, 1.0) if duration_ms > 0 else 0.0
        )

        metrics[block_id] = {
            "utilization": round(util, 4),
            "queue_depth": 0,
            "processed_count": max(0, processed - failures),
            "failures": failures,
        }

    return {"status": "complete", "metrics": metrics, "timeline": [], "errors": errors}


# ── Dispatch ──────────────────────────────────────────────────────────────────

def process(payload: Dict) -> Dict:
    project_json = payload.get("project_json") or {}
    scenario = payload.get("scenario") or {}
    block_behaviors = payload.get("block_behaviors") or {}

    try:
        if SIMPY_OK:
            return run_with_simpy(project_json, scenario, block_behaviors)
        return run_deterministic(project_json, scenario, block_behaviors)
    except Exception as exc:
        logging.exception("Simulation failed")
        return {
            "status": "error",
            "metrics": {},
            "timeline": [],
            "errors": [str(exc), traceback.format_exc()],
        }


def main() -> None:
    logging.info("simulation_engine ready")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
            output = process(payload)
            print(json.dumps(output), flush=True)
        except json.JSONDecodeError as exc:
            print(
                json.dumps(
                    {
                        "status": "error",
                        "metrics": {},
                        "timeline": [],
                        "errors": [f"JSON parse error: {exc}"],
                    }
                ),
                flush=True,
            )
        except Exception as exc:
            logging.exception("Unhandled error")
            print(
                json.dumps(
                    {
                        "status": "error",
                        "metrics": {},
                        "timeline": [],
                        "errors": [str(exc)],
                    }
                ),
                flush=True,
            )


if __name__ == "__main__":
    main()
