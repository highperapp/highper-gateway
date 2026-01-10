# Phase 2.3: GraphQL Gateway - Implementation Specification

**Duration:** 3 weeks
**Priority:** Medium-High
**Difficulty:** Medium-High
**Impact:** +3% API Gateway score

---

## Executive Summary

Implement a GraphQL gateway to enable GraphQL query routing, schema stitching, and federation across multiple backend GraphQL services. This allows clients to use a single GraphQL endpoint while the proxy federates queries to multiple backend services.

---

## What is a GraphQL Gateway?

### Traditional GraphQL
```
Client
   └──→ GraphQL API A (users, posts)
   └──→ GraphQL API B (comments, likes)

2 separate endpoints, manual federation
```

### GraphQL Gateway
```
Client
   └──→ GraphQL Gateway (unified schema)
          ├──→ Users Service (users, posts)
          ├──→ Social Service (comments, likes)
          └──→ Analytics Service (stats)

1 endpoint, automatic federation
```

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│            GraphQL Gateway System                    │
├─────────────────────────────────────────────────────┤
│                                                       │
│  Client GraphQL Query                                │
│       │                                              │
│       ↓                                              │
│  ┌─────────────────────────────────┐               │
│  │   Query Parser & Validator      │               │
│  │   - Parse GraphQL query         │               │
│  │   - Validate against schema     │               │
│  └─────────────────────────────────┘               │
│       │                                              │
│       ↓                                              │
│  ┌─────────────────────────────────┐               │
│  │   Query Planner                 │               │
│  │   - Split query by service      │               │
│  │   - Plan execution order        │               │
│  │   - Handle dependencies         │               │
│  └─────────────────────────────────┘               │
│       │                                              │
│       ↓                                              │
│  ┌─────────────────────────────────┐               │
│  │   Executor                      │               │
│  │   ├→ Service A (parallel)       │               │
│  │   ├→ Service B (parallel)       │               │
│  │   └→ Service C (after A+B)      │               │
│  └─────────────────────────────────┘               │
│       │                                              │
│       ↓                                              │
│  ┌─────────────────────────────────┐               │
│  │   Response Merger               │               │
│  │   - Merge subgraph results      │               │
│  │   - Apply transformations       │               │
│  └─────────────────────────────────┘               │
│       │                                              │
│       ↓                                              │
│  Combined GraphQL Response                          │
│                                                       │
└─────────────────────────────────────────────────────┘
```

---

## Configuration Schema

```rust
// File: highper-gateway/src/gateway/graphql/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLConfig {
    /// Enable GraphQL gateway
    pub enabled: bool,

    /// GraphQL endpoint path
    #[serde(default = "default_graphql_path")]
    pub path: String,

    /// Backend GraphQL services
    pub services: Vec<GraphQLService>,

    /// Schema stitching mode
    #[serde(default)]
    pub stitching_mode: StitchingMode,

    /// Enable GraphQL Playground
    #[serde(default = "default_true")]
    pub playground_enabled: bool,

    /// Enable introspection
    #[serde(default = "default_true")]
    pub introspection_enabled: bool,

    /// Query depth limit
    #[serde(default = "default_query_depth")]
    pub max_query_depth: usize,

    /// Query complexity limit
    #[serde(default = "default_query_complexity")]
    pub max_query_complexity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLService {
    /// Service name
    pub name: String,

    /// Service GraphQL endpoint URL
    pub url: String,

    /// Service schema (SDL)
    pub schema_url: Option<String>,

    /// Type extensions this service provides
    #[serde(default)]
    pub type_extensions: Vec<String>,

    /// Headers to forward
    #[serde(default)]
    pub forward_headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StitchingMode {
    /// Merge schemas into one
    Merge,

    /// Apollo Federation
    Federation,

    /// Manual stitching with resolvers
    Manual,
}

fn default_graphql_path() -> String {
    "/graphql".to_string()
}

fn default_true() -> bool {
    true
}

fn default_query_depth() -> usize {
    10
}

fn default_query_complexity() -> usize {
    1000
}
```

**Configuration Example:**

```yaml
graphql:
  enabled: true
  path: "/graphql"
  playground_enabled: true
  introspection_enabled: true
  max_query_depth: 10
  max_query_complexity: 1000
  stitching_mode: federation

  services:
    # Users service
    - name: "users"
      url: "http://users-service:4001/graphql"
      schema_url: "http://users-service:4001/graphql?sdl"
      type_extensions:
        - "User"
        - "Post"
      forward_headers:
        - "Authorization"
        - "X-User-ID"

    # Social service
    - name: "social"
      url: "http://social-service:4002/graphql"
      schema_url: "http://social-service:4002/graphql?sdl"
      type_extensions:
        - "Comment"
        - "Like"
      forward_headers:
        - "Authorization"

    # Analytics service
    - name: "analytics"
      url: "http://analytics-service:4003/graphql"
      type_extensions:
        - "Stats"
```

**Example Query:**

```graphql
# Client sends single query
query GetUserProfile {
  user(id: "123") {
    id
    name
    email

    # From users service
    posts {
      id
      title

      # From social service (stitched)
      comments {
        id
        text
        author {
          name
        }
      }

      # From social service
      likesCount
    }

    # From analytics service
    stats {
      postsCount
      followersCount
    }
  }
}

# Gateway splits into 3 queries:
# 1. users service: user, posts
# 2. social service: comments, likesCount (needs post IDs from #1)
# 3. analytics service: stats
```

---

## Implementation

### 1. GraphQL Handler

```rust
// File: highper-gateway/src/gateway/graphql/handler.rs

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptyMutation, EmptySubscription, Schema,
};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode};

pub struct GraphQLHandler {
    schema: Schema<Query, EmptyMutation, EmptySubscription>,
    playground_enabled: bool,
}

impl GraphQLHandler {
    pub fn new(config: &GraphQLConfig) -> anyhow::Result<Self> {
        // Build unified schema from services
        let schema = Self::build_schema(config)?;

        Ok(Self {
            schema,
            playground_enabled: config.playground_enabled,
        })
    }

    /// Handle GraphQL request
    pub async fn handle(
        &self,
        req: Request<Incoming>,
    ) -> anyhow::Result<Response<Full<Bytes>>> {
        match (req.method(), req.uri().path()) {
            // GraphQL queries
            (&Method::POST, path) if path.starts_with("/graphql") => {
                self.handle_graphql_query(req).await
            }

            // GraphQL Playground (dev tool)
            (&Method::GET, path) if path.starts_with("/graphql") && self.playground_enabled => {
                Ok(self.playground_response())
            }

            _ => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))?),
        }
    }

    /// Handle GraphQL query
    async fn handle_graphql_query(
        &self,
        req: Request<Incoming>,
    ) -> anyhow::Result<Response<Full<Bytes>>> {
        use http_body_util::BodyExt;

        // Parse request body
        let body = req.collect().await?.to_bytes();
        let graphql_request: async_graphql::Request = serde_json::from_slice(&body)?;

        // Execute query
        let response = self.schema.execute(graphql_request).await;

        // Serialize response
        let json = serde_json::to_string(&response)?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))?)
    }

    /// GraphQL Playground HTML
    fn playground_response(&self) -> Response<Full<Bytes>> {
        let html = playground_source(GraphQLPlaygroundConfig::new("/graphql"));

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(Full::new(Bytes::from(html)))
            .unwrap()
    }

    /// Build unified schema from services
    fn build_schema(
        config: &GraphQLConfig,
    ) -> anyhow::Result<Schema<Query, EmptyMutation, EmptySubscription>> {
        // Load schemas from all services
        let schemas = Self::fetch_service_schemas(&config.services)?;

        // Stitch schemas together
        let stitched = match config.stitching_mode {
            StitchingMode::Merge => Self::merge_schemas(&schemas)?,
            StitchingMode::Federation => Self::federate_schemas(&schemas)?,
            StitchingMode::Manual => Self::manual_stitching(&schemas)?,
        };

        // Build async-graphql schema
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .data(config.clone())
            .finish();

        Ok(schema)
    }

    fn fetch_service_schemas(
        services: &[GraphQLService],
    ) -> anyhow::Result<Vec<(String, String)>> {
        // Fetch SDL from each service
        let mut schemas = vec![];

        for service in services {
            if let Some(schema_url) = &service.schema_url {
                // Fetch schema via introspection or SDL endpoint
                let schema_sdl = Self::fetch_schema_sdl(schema_url)?;
                schemas.push((service.name.clone(), schema_sdl));
            }
        }

        Ok(schemas)
    }

    fn fetch_schema_sdl(url: &str) -> anyhow::Result<String> {
        // Send introspection query or fetch SDL
        // For now, placeholder
        Ok(String::new())
    }

    fn merge_schemas(schemas: &[(String, String)]) -> anyhow::Result<String> {
        // Simple merge - concatenate schemas
        let merged = schemas
            .iter()
            .map(|(_, schema)| schema.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(merged)
    }

    fn federate_schemas(schemas: &[(String, String)]) -> anyhow::Result<String> {
        // Apollo Federation - merge with @key directives
        // This requires parsing and analyzing federation directives
        Ok(String::new())
    }

    fn manual_stitching(schemas: &[(String, String)]) -> anyhow::Result<String> {
        // Manual stitching with custom resolvers
        Ok(String::new())
    }
}

/// Root query type
struct Query;

#[async_graphql::Object]
impl Query {
    /// Delegate to backend services
    async fn user(&self, ctx: &async_graphql::Context<'_>, id: String) -> User {
        // Get service config
        let config = ctx.data::<GraphQLConfig>().unwrap();

        // Find user service
        let user_service = config
            .services
            .iter()
            .find(|s| s.name == "users")
            .unwrap();

        // Forward query to user service
        Self::forward_to_service(user_service, "user", vec![("id", &id)]).await
    }

    async fn forward_to_service(
        service: &GraphQLService,
        field: &str,
        args: Vec<(&str, &str)>,
    ) -> User {
        // Build GraphQL query for backend
        let query = format!("query {{ {}({}) }}", field, /* args */);

        // Send to backend
        let client = reqwest::Client::new();
        let response = client
            .post(&service.url)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .unwrap();

        // Parse response
        response.json().await.unwrap()
    }
}

/// User type (simplified)
#[derive(async_graphql::SimpleObject)]
struct User {
    id: String,
    name: String,
    email: String,
}
```

### 2. Schema Stitching

```rust
// File: highper-gateway/src/gateway/graphql/stitching.rs

use graphql_parser::parse_schema;

pub struct SchemaStitcher;

impl SchemaStitcher {
    /// Stitch multiple GraphQL schemas into one
    pub fn stitch(schemas: Vec<String>) -> anyhow::Result<String> {
        let mut types = HashMap::new();
        let mut queries = vec![];
        let mut mutations = vec![];

        for schema_sdl in schemas {
            let doc = parse_schema::<String>(&schema_sdl)?;

            // Extract types
            for definition in doc.definitions {
                match definition {
                    Definition::TypeDefinition(type_def) => {
                        // Collect type definitions
                    }
                    Definition::SchemaDefinition(schema_def) => {
                        // Collect root types
                    }
                    _ => {}
                }
            }
        }

        // Merge types, handle conflicts
        // Generate unified schema SDL

        Ok(String::new())
    }
}
```

### 3. Query Federation

```rust
// File: highper-gateway/src/gateway/graphql/federation.rs

pub struct QueryFederator {
    services: Vec<GraphQLService>,
}

impl QueryFederator {
    /// Execute federated query across services
    pub async fn execute(&self, query: &str) -> anyhow::Result<serde_json::Value> {
        // Parse query
        let parsed = self.parse_query(query)?;

        // Analyze query to determine which services to call
        let execution_plan = self.plan_execution(&parsed)?;

        // Execute in parallel/sequential as needed
        let results = self.execute_plan(execution_plan).await?;

        // Merge results
        self.merge_results(results)
    }

    fn plan_execution(&self, query: &ParsedQuery) -> anyhow::Result<ExecutionPlan> {
        // Determine which services provide which fields
        // Create execution plan with dependencies

        Ok(ExecutionPlan::default())
    }

    async fn execute_plan(&self, plan: ExecutionPlan) -> anyhow::Result<Vec<ServiceResult>> {
        // Execute queries to services based on plan
        Ok(vec![])
    }

    fn merge_results(&self, results: Vec<ServiceResult>) -> anyhow::Result<serde_json::Value> {
        // Merge results from different services
        Ok(serde_json::Value::Null)
    }
}

#[derive(Default)]
struct ExecutionPlan {
    stages: Vec<ExecutionStage>,
}

struct ExecutionStage {
    service: String,
    query: String,
    depends_on: Vec<String>,
}

struct ServiceResult {
    service: String,
    data: serde_json::Value,
}
```

---

## Dependencies

```toml
[dependencies]
# GraphQL server
async-graphql = { version = "7.0", features = ["playground"] }

# GraphQL parsing
graphql-parser = "0.4"

# Apollo Federation (optional)
apollo-router-core = { version = "1.0", optional = true }

# Already have:
# serde_json = "1.0"
# reqwest = "0.11"
```

---

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_schema_stitching()

#[tokio::test]
async fn test_query_federation()

#[tokio::test]
async fn test_query_depth_limit()

#[tokio::test]
async fn test_query_complexity_limit()
```

### Integration Tests

```rust
#[tokio::test]
async fn test_graphql_gateway_end_to_end() {
    // Start mock GraphQL services
    let users_service = start_graphql_service(4001, r#"
        type Query {
            user(id: ID!): User
        }
        type User {
            id: ID!
            name: String!
        }
    "#);

    let social_service = start_graphql_service(4002, r#"
        type Query {
            comments(postId: ID!): [Comment!]!
        }
        type Comment {
            id: ID!
            text: String!
        }
    "#);

    // Start proxy with GraphQL gateway
    let proxy = start_proxy_with_graphql().await;

    // Query unified schema
    let query = r#"
        query {
            user(id: "123") {
                id
                name
            }
        }
    "#;

    let response = reqwest::Client::new()
        .post("http://localhost:8080/graphql")
        .json(&json!({ "query": query }))
        .send()
        .await
        .unwrap();

    let result: serde_json::Value = response.json().await.unwrap();
    assert_eq!(result["data"]["user"]["name"], "John");
}
```

---

## GraphQL Subscriptions (WebSocket)

```rust
// File: highper-gateway/src/gateway/graphql/subscriptions.rs

use async_graphql::http::WebSocketProtocols;

pub async fn handle_graphql_subscription(
    ws: WebSocket,
    schema: Schema<Query, EmptyMutation, Subscription>,
) {
    // Upgrade to WebSocket
    let protocol = WebSocketProtocols::GraphQLWS;

    // Handle subscription
    WebSocket::new(schema, ws, protocol)
        .on_connection_init(|value| async move {
            // Authentication from connection_init payload
            Ok(())
        })
        .serve()
        .await;
}
```

---

## Performance

### Caching

```rust
// Field-level caching
#[derive(async_graphql::SimpleObject)]
struct User {
    id: String,

    #[graphql(cache_control(max_age = 60))]
    name: String,

    #[graphql(cache_control(max_age = 3600))]
    email: String,
}
```

### DataLoader

```rust
// Batch loading to prevent N+1 queries
use async_graphql::dataloader::*;

struct PostLoader {
    client: reqwest::Client,
}

impl Loader<String> for PostLoader {
    type Value = Post;
    type Error = Arc<anyhow::Error>;

    async fn load(&self, keys: &[String]) -> Result<HashMap<String, Self::Value>, Self::Error> {
        // Batch fetch posts by IDs
        let posts = self.fetch_posts_batch(keys).await?;
        Ok(posts)
    }
}
```

---

## Acceptance Criteria

- [ ] GraphQL queries routed to backends
- [ ] Schema stitching works (merge mode)
- [ ] Federation works (Apollo Federation)
- [ ] Query depth/complexity limits enforced
- [ ] GraphQL Playground accessible
- [ ] Introspection works
- [ ] WebSocket subscriptions work
- [ ] Field-level caching works
- [ ] Unit tests pass
- [ ] Integration tests pass

---

## Documentation

Create `docs/GRAPHQL_GATEWAY.md`:
- GraphQL gateway overview
- Schema stitching guide
- Federation setup
- Query examples
- Performance tuning
- Troubleshooting

---

## Next Steps

1. Implement OpenTelemetry tracing (Phase 2.4)
2. Add GraphQL metrics
3. Optimize query execution

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
