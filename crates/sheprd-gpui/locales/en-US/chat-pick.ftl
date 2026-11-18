# Why a new chat starts with another agent than the one used last, or why
# the composer suggests another: the plan usage the status bar last read.
# $agent is an agent's name, such as Claude; $percent is how much of its
# tightest limit is used; $time is a countdown such as 2h 10m.
chat-pick-spent = { $agent } has hit its limit
chat-pick-spent-resets = { $agent } has hit its limit until it resets in { $time }
chat-pick-tight = { $agent } is { $percent }% used
chat-pick-tight-resets = { $agent } is { $percent }% used, resets in { $time }
chat-pick-running-out = { $agent } is { $percent }% used and runs out in { $time } at this pace
# The composer, after the Agent button started another agent; $why is one
# of the reasons above.
chat-pick-started-instead = Started with { $agent }: { $why }
# The composer, while the chat runs an agent whose plan is short; clicking
# switches to $other.
chat-pick-suggest = { $why }. Switch to { $other }
