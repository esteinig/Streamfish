

use tonic::{Request, Status};
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::Interceptor;

#[derive(Debug, Clone)]
pub struct AuthInterceptor {
    pub token: MetadataValue<Ascii>
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request.metadata_mut().insert("local-auth", self.token.clone());
        Ok(request)
    }
}
