"""
I'm not sure whether Dorado performs as well as the Guppy server with the (hacky) streaming input. There are plans to 
provide a (hopefully open-source) drop-in client for a Dorado server. All in all, it might be a good idea to at least
test a simple Guppy implementation that - unfortuantely - runs through Python, which implements the C++ client, and is
in this case called as a process script in the pipeline as drop-in for the default Dorado process IO.

Let's see if this works...
"""
