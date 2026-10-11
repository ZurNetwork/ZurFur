# Release stages

The Engineer's ruling, 2026-10-09. Every release tag carries its stage: `<stage>-v<major>.<minor>.<patch>`.

| Stage | Who sees it | What it means |
| --- | --- | --- |
| `pre-alpha-vx.x.x` | The Engineer only | Fast development. Breaking changes may happen. The proto stays tagged `v1`: a breaking change does not bump it. |
| `alpha-vx.x.x` | A closed group, on a real domain | The MVP is done or close to done. Users earn invite codes through usage, Bluesky-style, but more of them than Bluesky gave, because artists and their clients need to bring each other in. |
| `beta-vx.x.x` | A bigger closed group | The largest stage. Most things work. The post-MVP roadmap is built here. |
| `release-vx.x.x` | Everyone | The actual application, used for real work. |

The Engineer's words:

> pre-alpha-vx.x.x -> Only I check this. Fast development. Breaking changes may happen. We never change the tag of v1 for the proto.
> alpha-vx.x.x -> Release to an actual domain. Tested by a closed group. Implementation of Bsky-like Codes that users get through usage. Higher amount, because artists and people need to get each other there. MVP is done or close to done.
> beta-vx.x.x -> Bigger closed group. Largest stage. Most things work. Post-MVP roadmap.
> release-vx.x.x -> Actual application. Real life work.
