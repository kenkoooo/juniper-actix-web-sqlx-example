use std::env;

use actix_web::{get, route, web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use juniper::{
    graphql_object, http::GraphQLRequest, EmptySubscription, FieldResult, GraphQLInputObject,
    RootNode,
};
use juniper_actix::{graphiql_handler, playground_handler};
use sqlx::{postgres::PgRow, PgPool, Row};

struct Context {
    pool: PgPool,
}

impl juniper::Context for Context {}

struct User {
    id: i32,
    name: String,
}
#[juniper::graphql_object(Context = Context)]
impl User {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(GraphQLInputObject)]
struct UserInput {
    name: String,
}

struct Query;

#[graphql_object(context = Context)]
impl Query {
    async fn users(context: &Context) -> FieldResult<Vec<User>> {
        let users = sqlx::query("SELECT id, name FROM users")
            .try_map(|row: PgRow| {
                let id: i32 = row.try_get("id")?;
                let name: String = row.try_get("name")?;
                Ok(User { id, name })
            })
            .fetch_all(&context.pool)
            .await?;
        Ok(users)
    }

    async fn user(context: &Context, id: i32) -> FieldResult<User> {
        let user = sqlx::query("SELECT id, name FROM users WHERE id=$1")
            .bind(id)
            .try_map(|row: PgRow| {
                let id: i32 = row.try_get("id")?;
                let name: String = row.try_get("name")?;
                Ok(User { id, name })
            })
            .fetch_one(&context.pool)
            .await?;
        Ok(user)
    }
}

struct Mutation;

#[graphql_object(context = Context)]
impl Mutation {
    async fn create_user(context: &Context, input: UserInput) -> FieldResult<User> {
        let row = sqlx::query("INSERT INTO users (name) VALUES ($1) RETURNING id")
            .bind(&input.name)
            .fetch_one(&context.pool)
            .await?;
        let id = row.try_get("id")?;
        Ok(User {
            id,
            name: input.name,
        })
    }
}

type Schema = RootNode<'static, Query, Mutation, EmptySubscription<Context>>;

#[route("/graphql", method = "GET", method = "POST")]
async fn graphql(
    pool: web::Data<PgPool>,
    schema: web::Data<Schema>,
    data: web::Json<GraphQLRequest>,
) -> impl Responder {
    let ctx = Context {
        pool: pool.as_ref().clone(),
    };

    let res = data.execute(&schema, &ctx).await;
    HttpResponse::Ok().json(res)
}

#[get("/playground")]
async fn playground() -> impl Responder {
    playground_handler("/graphql", None).await
}
#[get("/graphiql")]
async fn graphiql() -> impl Responder {
    graphiql_handler("/graphql", None).await
}
#[actix_web::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;

    let db_url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&db_url).await?;

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(Schema::new(
                Query,
                Mutation,
                EmptySubscription::new(),
            )))
            .service(graphql)
            .service(playground)
            .service(graphiql)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;

    Ok(())
}
