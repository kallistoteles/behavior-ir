import Init

namespace Safety.Candidate

def literalBytes (s : String) : List Nat := s.toByteArray.data.toList.map UInt8.toNat

def compactPayload (s : String) : List (List Nat) := [literalBytes s]

theorem compactPayload_flatten (s : String) : (compactPayload s).flatten = literalBytes s := by
  simp [compactPayload]

theorem literalBytes_injective {a b : String} (h : literalBytes a = literalBytes b) : a = b := by
  apply String.toByteArray_inj.mp
  apply ByteArray.ext
  apply Array.toList_inj.mp
  exact (List.map_inj_right (fun x y eq => UInt8.toNat_inj.mp eq)).mp h

end Safety.Candidate
