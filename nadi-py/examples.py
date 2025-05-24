import nadi_py as nadi


net = nadi.Network.from_str("""
a -> b
c -> d
b -> d
""")

net.nodes

functions = nadi.NadiFunctions()

functions.env("sum", [1, 2.0])

fnsum = functions.env_function("sum")

fnsum([1,2,3])

fnsum.__signature__
fnsum.__text_signature__
