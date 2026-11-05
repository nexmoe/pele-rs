// Mermaid's swimlane specs, as ported by pele 1.9.0 in tests/compat/swimlane.
// Mermaid's own swimlane specs test the layout engine and the theme, not the syntax: the syntax
// is the flowchart's, and Mermaid's flowchart specs cover it. So the cases here are Mermaid's own
// swimlane source and the examples of Mermaid's documentation, which the corpus collects.
//
// The file is written by hand: `cargo xtask compat swimlane` has no spec files of pele's to read.
//
// Do not edit: `node tools/oracle/golden.mjs <pele checkout> swimlane` writes the golden file.

const CASES: &[Case] = &[
    Case {
        file: "swimlanes/swimlanesDiagram.spec.ts",
        name: "a swimlane with two nodes",
        src: "swimlane-beta TD\n  A --> B",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: the order that crosses three lanes",
        src: "swimlane-beta LR\n  subgraph Customer\n    Browse[Browse catalogue]\n    Pay[Pay]\n  end\n  subgraph Warehouse\n    Pick[Pick items]\n    Ship[Ship order]\n  end\n  subgraph Finance\n    Invoice[Raise invoice]\n  end\n  Browse --> Pay\n  Pay --> Pick\n  Pick --> Ship\n  Pay --> Invoice\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: the same order with a front matter",
        src: "---\nconfig:\n  theme: default\n  look: classic\n---\nswimlane-beta LR\n  subgraph Customer\n    Browse[Browse catalogue]\n    Pay[Pay]\n  end\n  subgraph Warehouse\n    Pick[Pick items]\n    Ship[Ship order]\n  end\n  subgraph Finance\n    Invoice[Raise invoice]\n  end\n  Browse --> Pay\n  Pay --> Pick\n  Pick --> Ship\n  Pay --> Invoice\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: a request that is escalated",
        src: "swimlane-beta LR\n  subgraph Customer\n    request[Request service]\n    receive[Receive update]\n  end\n\n  subgraph Support\n    triage[Triage request]\n    answer[Send answer]\n  end\n\n  subgraph Engineering\n    investigate[Investigate issue]\n    fix[Prepare fix]\n  end\n\n  request --> triage\n  triage -->|Known issue| answer\n  triage -->|Needs code change| investigate\n  investigate --> fix --> answer\n  answer --> receive\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: one lane and nothing else",
        src: "swimlane-beta\n  subgraph Sales\n    lead[Qualify lead]\n    quote[Prepare quote]\n  end\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: lanes with a title of their own",
        src: "swimlane-beta LR\n  subgraph sales [Sales team]\n    lead[Qualify lead]\n    quote[Prepare quote]\n  end\n\n  subgraph finance [Finance team]\n    review[Review terms]\n    approve[Approve quote]\n  end\n\n  lead --> quote --> review --> approve\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: a decision that sends the work back",
        src: "swimlane-beta LR\n  subgraph Intake\n    start([Start])\n    task[Do work]\n    fix[Fix issues]\n  end\n\n  subgraph Review\n    decision{Ready?}\n  end\n\n  subgraph Complete\n    done((Done))\n  end\n\n  start --> task --> decision\n  decision -->|Yes| done\n  decision -->|No| fix\n  fix --> task\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: an order that waits for its invoice",
        src: "swimlane-beta LR\n  subgraph Buyer\n    choose[Choose product]\n    pay[Pay invoice]\n  end\n\n  subgraph Store\n    reserve[Reserve stock]\n    ship[Ship product]\n  end\n\n  choose --> reserve\n  reserve -->|Invoice ready| pay\n  pay --> ship\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: a request with an accessible title",
        src: "swimlane-beta LR\n  accTitle: Support escalation\n  accDescr: A request starts with the customer, is triaged by support, and may be escalated to engineering.\n\n  subgraph Customer\n    request[Open request]\n  end\n\n  subgraph Support\n    triage[Triage]\n  end\n\n  subgraph Engineering\n    resolve[Resolve]\n  end\n\n  request --> triage --> resolve\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: an order from the customer to the carrier",
        src: "swimlane-beta LR\n  subgraph Customer\n    submit[Submit order]\n    confirm[Confirm delivery]\n  end\n\n  subgraph Store\n    check[Check order]\n    pack[Pack items]\n  end\n\n  subgraph Carrier\n    collect[Collect package]\n    deliver[Deliver package]\n  end\n\n  submit --> check --> pack --> collect --> deliver --> confirm\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: an application that comes back for changes",
        src: "swimlane-beta LR\n  subgraph Applicant\n    apply[Submit application]\n    sign[Sign agreement]\n  end\n\n  subgraph Reviewer\n    screen[Screen application]\n    decide{Approved?}\n  end\n\n  subgraph System\n    create[Create account]\n    notify[Send welcome email]\n  end\n\n  apply -->|Application received| screen\n  screen --> decide\n  decide -->|Approved| create --> notify --> sign\n  decide -->|Needs changes| apply\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: the same flow from top to bottom",
        src: "swimlane-beta TB\n  subgraph Intake\n    collect[Collect request]\n    validate[Validate details]\n  end\n\n  subgraph Review\n    review[Review request]\n    decide{Ready?}\n  end\n\n  subgraph Delivery\n    schedule[Schedule work]\n    complete[Complete work]\n  end\n\n  collect --> validate --> review --> decide\n  decide -->|Yes| schedule --> complete\n  decide -->|No| collect\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: lanes with a class of their own",
        src: "swimlane-beta LR\n  subgraph ops [Operations]\n    intake[Receive request]\n    plan[Plan work]\n  end\n\n  subgraph legal [Legal]\n    review[Review contract]\n  end\n\n  intake --> plan --> review\n\n  classDef attention fill:#fff2cc,stroke:#d6a500,color:#111;\n  class review attention;\n",
        todo: false,
    },
    Case {
        file: "swimlane-docs.json",
        name: "docs: a question that is answered by two lanes",
        src: "swimlane-beta LR\n  subgraph Support\n    classify{Can support solve it?}\n    respond[Respond to customer]\n  end\n\n  subgraph Product\n    prioritize[Prioritize fix]\n  end\n\n  subgraph Engineering\n    implement[Implement fix]\n  end\n\n  classify -->|Yes| respond\n  classify -->|No| prioritize --> implement --> respond\n",
        todo: false,
    },
];
