// Borrowed from https://github.com/rusty-crab/warp-api-starter-template (Thanks!)

use crate::{auth, error};
use http_api_problem::HttpApiProblem as Problem;
use std::convert::Infallible;
use warp::http;
use warp::{Rejection, Reply};

pub fn build<E: Into<anyhow::Error>>(err: E) -> Rejection {
    warp::reject::custom(pack(err.into()))
}

pub fn pack(err: anyhow::Error) -> Problem {
    let err = match err.downcast::<Problem>() {
        Ok(problem) => return problem,
        Err(err) => err,
    };

    if let Some(err) = err.downcast_ref::<auth::AuthError>() {
        match err {
            auth::AuthError::InvalidCredentials => {
                return Problem::new(http::StatusCode::UNAUTHORIZED).title("Invalid credentials.")
            }
        }
    }

    if let Some(err) = err.downcast_ref::<error::RequestError>() {
        match err {
            error::RequestError::PayloadTooLarge { limit } => {
                return Problem::new(http::StatusCode::PAYLOAD_TOO_LARGE)
                    .detail(format!("Payload must not exceed {} bytes", limit));
            }
        }
    }

    match err.downcast_ref::<biscuit::errors::Error>() {
        Some(biscuit::errors::Error::ValidationError(e)) => {
            return Problem::new(http::StatusCode::UNAUTHORIZED)
                .title("Invalid token.")
                .detail(format!("The provided auth token is invalid: {}", e));
        }
        Some(biscuit::errors::Error::DecodeError(e)) => {
            return Problem::new(http::StatusCode::UNAUTHORIZED)
                .title("Invalid token.")
                .detail(format!("Unable to decode auth token: {}", e));
        }
        Some(_) => (),
        None => (),
    }

    tracing::error!("internal error occurred: {:#}", err);
    Problem::with_title(http::StatusCode::INTERNAL_SERVER_ERROR)
}

fn reply_from_problem(problem: &Problem) -> impl Reply {
    let code = problem
        .status
        .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);

    let reply = warp::reply::json(problem);
    let reply = warp::reply::with_status(reply, code);
    warp::reply::with_header(
        reply,
        http::header::CONTENT_TYPE,
        http_api_problem::PROBLEM_JSON_MEDIA_TYPE,
    )
}

pub async fn unpack(rejection: Rejection) -> Result<impl Reply, Infallible> {
    let reply = if rejection.is_not_found() {
        let problem = Problem::with_title(http::StatusCode::NOT_FOUND);
        reply_from_problem(&problem)
    } else if let Some(problem) = rejection.find::<Problem>() {
        reply_from_problem(problem)
    } else if let Some(e) = rejection.find::<warp::filters::body::BodyDeserializeError>() {
        let problem = Problem::new(http::StatusCode::BAD_REQUEST)
            .title("Invalid request body.")
            .detail(format!("Request body is invalid: {}", e));
        reply_from_problem(&problem)
    } else if rejection.find::<warp::reject::MethodNotAllowed>().is_some() {
        let problem = Problem::with_title(http::StatusCode::METHOD_NOT_ALLOWED);
        reply_from_problem(&problem)
    } else {
        tracing::error!("Rejection: {:?}", rejection);
        let problem = Problem::with_title(http::StatusCode::INTERNAL_SERVER_ERROR);
        reply_from_problem(&problem)
    };

    Ok(reply)
}
