export type AxonixIR = {
  source: SourceNode;
  transforms: TransformNode[];
  view: ViewNode;
};

export type SourceNode = {
  kind: {
    Collection: {
      name: string;
    };
  };
};

export type TransformNode = {
  kind: {
    Grid: {
      columns: number;
    };
  };
};

export type ViewNode = {
  kind:
    | {
        Card: null;
      }
    | {
        Named: {
          name: string;
        };
      };
};

export class AxBuilder {
  private readonly collection: string;
  private readonly transforms: TransformNode[] = [];
  private viewNode: ViewNode = { kind: { Card: null } };

  constructor(collection: string) {
    this.collection = collection;
  }

  grid(columns = 3): this {
    this.transforms.push({ kind: { Grid: { columns } } });
    return this;
  }

  card(): this {
    this.viewNode = { kind: { Card: null } };
    return this;
  }

  view(name: string): this {
    if (name === "Card") {
      return this.card();
    }
    this.viewNode = { kind: { Named: { name } } };
    return this;
  }

  toIR(): AxonixIR {
    return {
      source: {
        kind: {
          Collection: {
            name: this.collection,
          },
        },
      },
      transforms: this.transforms,
      view: this.viewNode,
    };
  }

  toJSON(space = 2): string {
    return JSON.stringify(this.toIR(), null, space);
  }
}

export function from(collection: string): AxBuilder {
  return new AxBuilder(collection);
}

