% Erlang syntax fixture
% Multi-line comment
% continues here

-module(greeter).
-behaviour(gen_server).
-export([hello/0, start_link/0, init/1]).
-import(io, [format/1, format/2]).
-compile(export_all).

-include("records.hrl").
-define(TIMEOUT, 5000).

-record(state, {name :: string(), count :: integer()}).

hello() ->
  Count = 42,
  Name = "Ada",
  io:format("hello ~s~n", [Name]),
  Flag = true,
  Letter = $x,
  {ok, Count}.

start_link() ->
  gen_server:start_link({local, ?MODULE}, ?MODULE, [], []).

init([]) ->
  State = #state{name = "Ada", count = 42},
  {ok, State, ?TIMEOUT}.

handle_call(get_count, _From, State) ->
  {reply, State#state.count, State}.

handle_cast({set_name, Name}, State) ->
  {noreply, State#state{name = Name}}.

terminate(_Reason, _State) ->
  io:format("goodbye~n"),
  ok.

-ifdef(DEBUG).
-define(LOG(X), io:format("DEBUG: ~p~n", [X])).
-else.
-define(LOG(X), ok).
-endif.

factorial(0) -> 1;
factorial(N) when N > 0 -> N * factorial(N - 1).

list_length([]) -> 0;
list_length([_|T]) -> 1 + list_length(T).
