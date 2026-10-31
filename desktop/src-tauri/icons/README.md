# NPi Desktop — Application Icon

`icon-source.svg` is the vector master (also `branding/npi-icon.svg`). The PNG,
`.icns` and `.ico` files here are generated from a 1024px render with a
**transparent** background on the macOS icon grid (824px body, 100px margin, soft
shadow in the margin): `npx tauri icon <1024px.png> -o <empty-dir>`, then the files
that already exist in this folder are replaced by the ones in `<empty-dir>`.

The mark: a classical portico (a π read as lintel and two columns) with one
person-sized dot between the columns, ivory on near-black.
