% Erlang syntax fixture
-module(greeter).
-export([hello/0]).
hello() ->
  Count = 42,
  io:format("hello~n").
  ok.
