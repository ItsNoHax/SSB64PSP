# Matrix refresh — signed-clamp UV (RE-262)

Part of [docs/visual-regression/README.md](../README.md).

## Signed-clamp UV regression refresh (RE-262)

N64 vertex UVs are signed S10.5, but the PSP GE reads
`GU_TEXTURE_16BIT` as unsigned. Negative coordinates therefore retained
repeat phase but clamped to the wrong edge. Pack v29 marks only the affected
authored-UV primitives and `meshdraw` submits those corners as float UVs.

The complete PPSSPP-software matrix was rebuilt and visually reviewed. The
semantic deltas against the pre-fix goldens are:

| Scene | Differing pixels | Post-RE-262 SHA-256 |
|---|---:|---|
| Dream Land | 23,852 | `06985d0fce8671b6cb1d66018834653b95380ff9a5e8164f429c9a29ebe9e60d` |
| Opening room | 1,096 | `148d9d1ad9161a988c91d579fa54908424eac471f1272793547e5cda34b8ac59` |
| Sector | 1,512 | `646d177fa69c46d832036b63e83ff80195f0145b26dbd016f37b3dc15266fc8c` |
| Saffron City | 2,428 | `91e63ab2a2b4d34a870c44e8d9c1970e82735d62f3cc951b83fe55cb8f437738` |
| Fox | 9,744 | `3196cc914976b273fce444e75deadd5307ced52ac65b735b89ea2f71fabf6a2f` |
| Captain Falcon | 21,152 | `e2cd218f2fd650a2caa9e89775da9c8b0d7370898b9e717f6f307672062c41a1` |
| Kirby | 804 | `019f2da6889c1c3bf80f5f93dd26e2d9d91d28193405600c1603bff7236e9051` |
| Ness | 5,420 | `3816230c761b9f37537218585fac604bec621dcced00a5f1a405b5a287942835` |
| Donkey Kong | 11,448 | `b9bc2f20f59201421c69c832a04a8c84a923f3fe909ad22edf43e9ef7ea127ec` |
| Mixed linear-texgen graph | 8,316 | `c23fc59e5ddb82de9418c923272a228e947fc7e3216dc985383dbf2b5610684a` |
| Link | 2,784 | `b2a6763d4670475df115b396773fe3c2a9a7858ee904be9ed223636445124b54` |

The flat-colour scene, three pure ordinary-texgen controls, and synthetic
depth-mask diagnostic remain byte-identical. Scene 8's change is confined to
neighboring authored-UV rail primitives; its linear-generated crystal remains
on the established path. Two Fox captures and two Link captures independently
match byte-for-byte. All changed regions replace stretched edge/blank texels
with coherent source art.

The physical-PSP comparisons recorded in the older per-scene sections predate
this refresh. They remain evidence for the renderer state tested at the time,
but do not validate these 11 current goldens; re-capture is still required by
R2.
