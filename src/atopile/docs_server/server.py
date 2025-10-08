"""Simple HTTP server for Atopile documentation."""

import http.server
import logging
import os
import socketserver
import webbrowser
from pathlib import Path
from urllib.parse import urlparse

logger = logging.getLogger(__name__)


class DocHandler(http.server.SimpleHTTPRequestHandler):
    """Custom handler for serving documentation."""
    
    def __init__(self, *args, directory=None, **kwargs):
        self.docs_dir = Path(directory) if directory else Path.cwd()
        super().__init__(*args, directory=directory, **kwargs)
        
    def do_GET(self):
        """Handle GET requests with custom routing."""
        parsed_path = urlparse(self.path)
        path = parsed_path.path
        
        # Route module pages
        if path.startswith('/module/'):
            module_path = path[8:]  # Remove '/module/'
            file_path = self.docs_dir / 'module' / f"{module_path}.html"
            
            if file_path.exists():
                self.path = f"/module/{module_path}.html"
            else:
                self.send_error(404, "Module not found")
                return
                
        # Redirect root to index
        elif path == '/':
            self.path = '/index.html'
            
        # Let parent class handle the rest
        super().do_GET()
        
    def end_headers(self):
        """Add custom headers."""
        # Add cache control for development
        self.send_header('Cache-Control', 'no-cache, no-store, must-revalidate')
        self.send_header('Pragma', 'no-cache')
        self.send_header('Expires', '0')
        super().end_headers()


class DocServer:
    """Documentation server."""
    
    def __init__(self, docs_dir: Path, port: int = 8080):
        self.docs_dir = docs_dir
        self.port = port
        self.httpd = None
        
    def serve(self, open_browser: bool = True):
        """Start the documentation server."""
        os.chdir(self.docs_dir)
        
        # Create handler with custom directory
        handler_class = lambda *args, **kwargs: DocHandler(
            *args, 
            directory=str(self.docs_dir),
            **kwargs
        )
        
        with socketserver.TCPServer(("", self.port), handler_class) as httpd:
            self.httpd = httpd
            
            url = f"http://localhost:{self.port}"
            logger.info(f"Serving documentation at {url}")
            print(f"\n📚 Documentation server running at {url}")
            print("Press Ctrl+C to stop the server\n")
            
            if open_browser:
                webbrowser.open(url)
                
            try:
                httpd.serve_forever()
            except KeyboardInterrupt:
                print("\nShutting down documentation server...")
                
    def stop(self):
        """Stop the server."""
        if self.httpd:
            self.httpd.shutdown()