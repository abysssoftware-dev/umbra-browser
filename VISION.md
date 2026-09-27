# Umbra Browser Architecture
Base: LibreWolf / Firefox

## Core Objectives
1. Replace LibreWolf/Firefox branding with Umbra assets.
2. Build custom onboarding UI.
3. Pre-install and enforce uBlock Origin, AdNauseam, and TrackMeNot.
4. Maximize privacy and security hardening.

## Phase 1: Codebase Initialization
- Clone LibreWolf source.
- Map the build environment.

## Phase 2: Branding Injection
- Locate branding subdirectories.
- Replace logos, icons, and product names.

## Phase 3: Extension Bundling
- Download target extension source files.
- Configure distribution folders to preload extensions.

## Phase 4: Onboarding UI
- Modify about:welcome experience.
- Apply custom CSS themes.
