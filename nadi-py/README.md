This can be installed from `pypi` with `pip install nadi-py` command.

Then you can simply import and use it:

```python
import nadi

net = nadi.Network.from_str("a -> b")
print([n.NAME for n in net.nodes])
```

The functions are available inside the `nadi.functions` submodule.

```python
import nadi
import nadi.functions as fn

net = nadi.Network.from_str("a -> b")
fn.network.svg_save(net, "test.svg")
```

Refer to examples in [Nadi Book](https://nadi-system.github.io/python.html) for more details.
