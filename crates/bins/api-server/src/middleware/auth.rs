use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::routes::{authenticate_headers, ApiState, AuthenticatedUser};

/// Authenticate every route in the protected router before its handler runs.
///
/// The authenticated principal is inserted into request extensions so handlers
/// can migrate away from repeated bearer-token parsing without changing the
/// external API contract. The current handlers may still perform their own
/// capability/tenant checks and, during migration, may revalidate the token;
/// the route-layer guard is the security boundary that prevents a newly-added
/// protected route from accidentally becoming public.
pub async fn authenticate_request(
    State(state): State<ApiState>,
    mut request: Request,
    next: Next,
) -> Response {
    let authenticated = match authenticate_headers(&state, request.headers()).await {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };

    request
        .extensions_mut()
        .insert::<AuthenticatedUser>(authenticated);
    next.run(request).await
}
