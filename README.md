Hacash Fullnode
===



### Module Architecture

```
- x16rs    ->  x16rs-sys
- sys      ->  -
- field    ->  sys
- basis    ->  field
- protocol ->  basis
- chain    ->  protocol
- scaner   ->  protocol
- mint     ->  protocol
- node     ->  protocol, tokio
- server   ->  mint, tokio
- app      ->  mint
```

