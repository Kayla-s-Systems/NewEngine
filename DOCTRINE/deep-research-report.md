# Architecture Research Notes

This document captures the engineering rationale behind CoreEngine's host/plugin model.

## Service registry discipline

A service registry can hide dependencies if it is used as an unstructured global lookup table. CoreEngine mitigates that risk with descriptor metadata, declarative startup requirements, typed adapters, and gateway diagnostics.

## Capability matrix

Every provider should declare the capabilities it provides and the gateway it serves. Required capabilities are validated by profile policy. Optional capabilities degrade cleanly when unavailable.

## ABI stability

Runtime plugins cross an ABI boundary. Versioned service contracts and explicit descriptor metadata are required for compatibility and safe evolution.

## Deterministic frame pipeline

The runtime should preserve explicit frame stages and stable ordering. Controller systems produce intents, and dedicated apply stages own writes to shared state.

## Gateway facade

The gateway layer lets consumers depend on `engine.render`, `engine.physics`, `engine.assets`, or `engine.input` while providers keep their own service ids.
