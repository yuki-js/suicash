# SuiCash Mascot

Based on the requirements in `.opencode/skills/suicash/skill.md`.

> ## Mascot character
> A chimera of a chiibuki and a penguin.
> - It inherits the unsettling nature of a chiibuki without using purple.
> - The shape is not meant to match exactly.

| File | |
| --- | --- |
| `suicash-mascot.svg` | Main mascot artwork |

## What is it a chimera of?

**chiibuki** — nanka chiisakute bukimi na yatsu. A tiny, creepy little thing .

**Penguin** — a penguin-like character from a world inhabited by those strangely uncanny little creatures.

## Transferring the “mechanism” of the uncanny feeling

Rather than tracing the shape directly, I broke down why a chiibuki feels eerie and transferred that logic into this design.

1. **A formless blob with an extremely small face attached low on it** …
   This creates a huge blank area above the face and makes the boundary between the head and body disappear.
2. **The face does not blend into the body and instead floats as a white “mask”** …
   I fused this with the penguin’s white belly so that the belly and face become one continuous white region.
3. **The mouth is split much wider than the face** …
   The mouth is wider than the distance between the eyes.
4. **The hands and feet are degenerate protrusions** …
   They feel like skeletal remnants, with four unevenly sized limbs hanging down.

Additionally, the left and right sides are slightly unbalanced (eye size and height, asymmetry in the smile, ear length and angle, and leg length). Avoiding perfect bilateral symmetry creates an awkward sitting posture.

## The core of the chimera: making the ears into flippers

The defining feature of a chiibuki is its long ears, and the defining feature of a penguin is its flippers. By combining these two, I made the **ears into flipper-shaped extensions**. This makes the chimera immediately recognizable while also satisfying the requirement that the shape not match exactly.

From the penguin side, I also borrowed the white belly, the small beak, and the 3-toed webbed feet. The coexistence of a beak and a split mouth is part of the uncanny quality of the chimera.

## Response to the specification

| Specification | Response |
| --- | --- |
| Do not use purple | I used Sui blue `#4DA2FF` as the main color, with deep navy `#0A1A2F` for the outline and white for the belly and face. It uses the same palette as the logo. |
| The shape must not match exactly | A chiibuki has a tall, teardrop shape with ears rising straight upward. This version instead uses a **low-center-of-gravity, wide bag-like body**; the ears are **flipper-shaped and asymmetrical**, and the face is positioned lower. |

## Color palette

| | |
| --- | --- |
| Body | `#4DA2FF` |
| Outline | `#0A1A2F` |
| Belly / face | `#FFFFFF` |
| Tongue | `#8FD6E8` |

It is kept within the same palette as the logo in `assets/logo/`.

## How it was built

The outline is drawn by first laying down a thick deep-navy stroke and then overlaying a thin cyan stroke. This is used for the ears, legs, and webbed feet so adjacent contours blend cleanly. The body and face are filled shapes with stroke outlines.

The `viewBox` is adjusted to a square based on measured ink bounds.

## Exporting PNGs

The SVG is the source of truth. PNGs are generated artifacts and are not tracked in git (`.gitignore` is already configured).

```bash
npm --prefix assets install
npm --prefix assets run generate-png
```

See the “Exporting PNGs” section in `assets/logo/README.md` for details.

