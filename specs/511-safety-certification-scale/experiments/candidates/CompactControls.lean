import CompactPayload
open Safety.Candidate
example (s : String) : (compactPayload s).flatten = literalBytes s := compactPayload_flatten s
example : literalBytes "å" = [195,165] := by decide
example : literalBytes "quote\"\\" = [113,117,111,116,101,34,92] := by decide
example (a b : String) (h : literalBytes a = literalBytes b) : a = b := literalBytes_injective h
example : literalBytes "a" ≠ literalBytes "b" := by decide
