@0xb8d4d3e5f6a7c9d1;

struct FullTerm {
  docId @0 :UInt64;
  fieldMask @1 :UInt128;
  frequency @2 :UInt64;
}

struct Block {
  fullTerms @0 :List(FullTerm);
}

struct UInt128 {
  high @0 :UInt64;
  low @1 :UInt64;
}
