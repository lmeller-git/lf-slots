1

Currently the pool is not linearizable:
during a sequential scan of a buffer, the reader may conclude that the pool is empty even though there exists no linearizable point where the pool was truly empty.

To fix this we need to introduce an epoch tracking system with a double-collect fallback on pop(). This is slow.

Other options include completely rewriting this to use a treiber stack, or different base memory layouts that lend themselves better to linearizable sweeps.


2

use mpmc-resize to add dynamically resizabel pool variants
