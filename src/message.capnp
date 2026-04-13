@0x901224821dd830b6;

struct Color {
    r @0 :UInt8;
    g @1 :UInt8;
    b @2 :UInt8;
}

struct RoundConfig {
    difficulty @0 :UInt8;
    remaining @1 :UInt8;
    secret @2 :Color;
    # in millis
    now @3 :Int64;
    restart @4 :Bool;
}

struct Message {
  union {
    hello @0 :Void;
    roundConfig @1 :RoundConfig;
    guessReq :group {
        client @2 :Text;
        rgb @3 :Color;
    }
    guessRes :group {
        client @4 :Text;
        rgb @5 :Color;
        closeness @6 :UInt8;
        closest @7 :Bool;
    }
  }
}
