"""HTML documentation generator for Atopile projects."""

import json
import logging
import re
from pathlib import Path
from typing import Dict, List, Any
import html

from jinja2 import Environment, FileSystemLoader
try:
    from pygments import highlight
    from pygments.lexers import get_lexer_by_name
    from pygments.formatters import HtmlFormatter
    PYGMENTS_AVAILABLE = True
except ImportError:
    PYGMENTS_AVAILABLE = False

from atopile import front_end
from atopile.datatypes import TypeRef

logger = logging.getLogger(__name__)


class DocGenerator:
    """Generate HTML documentation from Atopile modules."""
    
    def __init__(self, project_path: Path, output_dir: Path):
        self.project_path = project_path
        self.output_dir = output_dir
        self.modules = []
        self.files = []
        
        # Setup Jinja2
        template_dir = Path(__file__).parent / "templates"
        self.env = Environment(loader=FileSystemLoader(template_dir))
        
    def generate(self):
        """Generate complete documentation."""
        logger.info(f"Generating documentation for {self.project_path}")
        
        # Create output directory
        self.output_dir.mkdir(parents=True, exist_ok=True)
        static_dir = self.output_dir / "static"
        static_dir.mkdir(exist_ok=True)
        
        # Copy static files
        self._copy_static_files()
        
        # Scan for .ato files
        self._scan_project()
        
        # Build inheritance relationships
        self._build_inheritance_chains()
        
        # Make imports clickable
        for module in self.modules:
            if 'imports' in module and module['imports']:
                module['imports'] = self._make_imports_clickable(module['imports'])
        
        # Generate pages
        self._generate_index()
        self._generate_module_pages()
        
        logger.info(f"Documentation generated in {self.output_dir}")
        
    def _copy_static_files(self):
        """Copy CSS and other static files."""
        import shutil
        static_src = Path(__file__).parent / "templates" / "static"
        static_dst = self.output_dir / "static"
        
        for file in static_src.glob("*"):
            shutil.copy2(file, static_dst / file.name)
            
    def _scan_project(self):
        """Scan project for .ato files and extract modules."""
        ato_files = list(self.project_path.rglob("*.ato"))
        
        for ato_file in ato_files:
            relative_path = ato_file.relative_to(self.project_path)
            
            # Try to parse the file
            try:
                modules_dict = front_end.bob.try_build_all_from_file(ato_file)
                
                file_info = {
                    'path': str(relative_path),
                    'module_count': len(modules_dict)
                }
                self.files.append(file_info)
                
                # Extract module information
                for ref, node in modules_dict.items():
                    module_data = self._extract_module_data(ato_file, ref, node)
                    module_data['file_path'] = str(relative_path)
                    module_data['path'] = f"{relative_path.stem}/{ref}"
                    self.modules.append(module_data)
                    
            except Exception as e:
                logger.warning(f"Failed to parse {relative_path}: {e}")
                # Fall back to basic parsing
                self._extract_basic_modules(ato_file, relative_path)
                
    def _extract_module_data(self, file_path: Path, ref: TypeRef, node) -> Dict[str, Any]:
        """Extract module data for documentation."""
        # Basic info
        data = {
            'name': str(ref),
            'type': type(node).__name__.replace('_driver', '').title(),
            'icon': self._get_icon_for_type(type(node).__name__),
            'docstring': None,
            'pins': [],
            'signals': [],
            'parameters': [],
            'instances': [],
            'connections': [],
            'assertions': [],
            'source_code': None,
            'parent_module': None,       # Direct parent from 'from X' syntax
            'inheritance_chain': [],     # Full chain of inheritance
            'imports': [],              # Import statements used
            'used_by': []              # Modules that inherit from this one
        }
        
        # Try to get source code snippet and inheritance info
        try:
            with open(file_path, 'r') as f:
                content = f.read()
                # Parse inheritance and imports
                data.update(self._parse_file_metadata(content, str(ref)))
                
                # Extract and format source code
                lines = content.split('\n')
                for i, line in enumerate(lines):
                    if f"module {ref}" in line or f"interface {ref}" in line:
                        # Get the module definition  
                        indent = len(line) - len(line.lstrip())
                        source_lines = [line]
                        
                        for j in range(i + 1, len(lines)):
                            next_line = lines[j]
                            if next_line.strip() and len(next_line) - len(next_line.lstrip()) <= indent:
                                break
                            source_lines.append(next_line)
                        
                        # Limit to 50 lines and format with syntax highlighting
                        raw_source = '\n'.join(source_lines[:50])
                        data['source_code'] = self._format_source_code(raw_source)
                        
                        # Parse module contents
                        data.update(self._parse_module_contents(source_lines))
                        break
        except Exception:
            pass
            
        return data
        
    def _extract_basic_modules(self, file_path: Path, relative_path: Path):
        """Extract basic module info when full parsing fails."""
        try:
            with open(file_path, 'r') as f:
                content = f.read()
                
            lines = content.split('\n')
            
            for i, line in enumerate(lines):
                stripped = line.strip()
                if stripped.startswith(('module ', 'interface ', 'component ')):
                    parts = stripped.split()
                    if len(parts) >= 2:
                        block_type = parts[0]
                        name = parts[1].rstrip(':')
                        
                        # Look for docstring
                        docstring = None
                        if i + 1 < len(lines):
                            next_line = lines[i + 1].strip()
                            if next_line.startswith(('"""', "'''")):
                                docstring = next_line.strip('"\'')
                        
                        # Parse inheritance for this module
                        parent_module = None
                        if ' from ' in stripped:
                            parts = stripped.split(' from ')
                            if len(parts) == 2:
                                parent_module = parts[1].rstrip(':').strip()
                        
                        module_data = {
                            'name': name,
                            'type': block_type.title(),
                            'icon': self._get_icon_for_type(block_type),
                            'file_path': str(relative_path),
                            'path': f"{relative_path.stem}/{name}",
                            'docstring': docstring,
                            'pins': [],
                            'signals': [],
                            'parameters': [],
                            'instances': [],
                            'connections': [],
                            'assertions': [],
                            'source_code': None,
                            'parent_module': parent_module,
                            'inheritance_chain': [],
                            'imports': self._parse_file_metadata(content, name)['imports'],
                            'used_by': []
                        }
                        
                        self.modules.append(module_data)
                        
        except Exception as e:
            logger.error(f"Failed to extract basic modules from {relative_path}: {e}")
            
    def _get_icon_for_type(self, type_name: str) -> str:
        """Get emoji icon for module type."""
        icons = {
            'module': '🔧',
            'interface': '🔌',
            'component': '📦',
            'Module': '🔧',
            'Interface': '🔌',
            'Component': '📦',
            'App': '📱'
        }
        return icons.get(type_name, '📄')
    
    def _format_source_code(self, source_code: str) -> str:
        """Format source code with syntax highlighting."""
        if not source_code:
            return ""
            
        if PYGMENTS_AVAILABLE:
            try:
                # Use Python lexer as closest match for .ato syntax
                lexer = get_lexer_by_name('python')
                formatter = HtmlFormatter(
                    style='default',
                    cssclass='highlight',
                    noclasses=True,
                    nowrap=False,  # Keep line structure
                    linenos=False
                )
                highlighted = highlight(source_code, lexer, formatter)
                # Remove the outer <div> wrapper that Pygments adds, keep inner content
                if highlighted.startswith('<div class="highlight"'):
                    # Find the end of the opening div tag and start of closing div tag
                    start_pos = highlighted.find('>') + 1
                    end_pos = highlighted.rfind('</div>')
                    if start_pos > 0 and end_pos > start_pos:
                        highlighted = highlighted[start_pos:end_pos]
                return highlighted
            except Exception as e:
                logger.warning(f"Failed to highlight source code: {e}")
                
        # Fallback: escape HTML and preserve formatting
        return html.escape(source_code)
    
    def _parse_module_contents(self, source_lines: List[str]) -> Dict[str, Any]:
        """Parse module contents to extract pins, signals, parameters, etc."""
        result = {
            'pins': [],
            'signals': [],
            'parameters': [],
            'instances': [],
            'connections': [],
            'assertions': []
        }
        
        for line in source_lines:
            stripped = line.strip()
            if not stripped or stripped.startswith('#') or stripped.startswith('"""') or stripped.startswith("'''"):
                continue
                
            # Parse pins: pin 1, pin "name", pin variable
            if stripped.startswith('pin '):
                pin_def = stripped[4:].split('~')[0].strip()  # Remove connections
                result['pins'].append({
                    'name': pin_def,
                    'line': stripped
                })
                
            # Parse signals: signal name ~ pin X
            elif stripped.startswith('signal '):
                parts = stripped.split('~')
                signal_name = parts[0][7:].strip()  # Remove 'signal '
                connection = parts[1].strip() if len(parts) > 1 else None
                result['signals'].append({
                    'name': signal_name,
                    'connection': connection,
                    'line': stripped
                })
                
            # Parse parameters: variable = value, variable: type = value
            elif '=' in stripped and not stripped.startswith(('import ', 'from ', 'assert ', 'module ', 'interface ')):
                if ' = ' in stripped:
                    parts = stripped.split(' = ', 1)
                    param_def = parts[0].strip()
                    value = parts[1].strip()
                    
                    # Check for type annotation
                    param_type = None
                    if ':' in param_def:
                        name_type = param_def.split(':', 1)
                        param_name = name_type[0].strip()
                        param_type = name_type[1].strip()
                    else:
                        param_name = param_def
                        
                    result['parameters'].append({
                        'name': param_name,
                        'type': param_type,
                        'value': value,
                        'line': stripped
                    })
                    
            # Parse instances: variable = new Type
            elif ' = new ' in stripped:
                parts = stripped.split(' = new ', 1)
                instance_name = parts[0].strip()
                instance_type = parts[1].strip()
                
                # Handle templated types: Type<param=value>
                if '<' in instance_type:
                    base_type = instance_type.split('<')[0]
                    template_params = instance_type[instance_type.find('<')+1:instance_type.rfind('>')]
                else:
                    base_type = instance_type
                    template_params = None
                    
                # Handle arrays: Type[count]
                array_size = None
                if '[' in base_type:
                    base_type, array_part = base_type.split('[', 1)
                    array_size = array_part.rstrip(']')
                    
                result['instances'].append({
                    'name': instance_name,
                    'type': base_type,
                    'array_size': array_size,
                    'template_params': template_params,
                    'line': stripped
                })
                
            # Parse connections: a ~ b, a ~> b, a <~ b
            elif any(op in stripped for op in [' ~ ', ' ~> ', ' <~ ']):
                # Skip signal definitions (already handled above)
                if not stripped.startswith('signal '):
                    result['connections'].append({
                        'line': stripped
                    })
                    
            # Parse assertions: assert condition
            elif stripped.startswith('assert '):
                assertion = stripped[7:].strip()  # Remove 'assert '
                result['assertions'].append({
                    'condition': assertion,
                    'line': stripped
                })
                
        return result
    
    def _parse_file_metadata(self, content: str, module_name: str) -> Dict[str, Any]:
        """Parse imports and inheritance from file content."""
        result = {
            'parent_module': None,
            'imports': []
        }
        
        lines = content.split('\n')
        
        for line in lines:
            stripped = line.strip()
            
            # Parse imports
            if stripped.startswith('import ') or stripped.startswith('from '):
                result['imports'].append({
                    'line': stripped,
                    'modules': self._extract_imported_modules(stripped)
                })
            
            # Parse inheritance for this specific module
            if f"module {module_name} from " in stripped or f"interface {module_name} from " in stripped:
                # Extract parent module name
                parts = stripped.split(' from ')
                if len(parts) == 2:
                    parent = parts[1].rstrip(':').strip()
                    result['parent_module'] = parent
        
        return result
    
    def _extract_imported_modules(self, import_line: str) -> List[str]:
        """Extract module names from import statements."""
        modules = []
        
        if import_line.startswith('from '):
            # from "path" import Module1, Module2
            if ' import ' in import_line:
                import_part = import_line.split(' import ', 1)[1]
                module_names = [name.strip() for name in import_part.split(',')]
                modules.extend(module_names)
        elif import_line.startswith('import '):
            # import Module1, Module2
            import_part = import_line[7:]  # Remove 'import '
            module_names = [name.strip() for name in import_part.split(',')]
            modules.extend(module_names)
            
        return modules
    
    def _make_imports_clickable(self, imports: List[Dict]) -> List[Dict]:
        """Make import statements clickable by linking to known modules."""
        # Create a lookup of module names to their paths
        module_lookup = {}
        for module in self.modules:
            module_lookup[module['name']] = module.get('path', '')
            
        clickable_imports = []
        for import_info in imports:
            clickable_line = import_info['line']
            
            # Replace module names with links if they exist
            for module_name in import_info['modules']:
                if module_name in module_lookup and module_lookup[module_name]:
                    link = f'<a href="/module/{module_lookup[module_name]}" class="import-link">{module_name}</a>'
                    # Replace the module name in the line (be careful with partial matches)
                    pattern = r'\b' + re.escape(module_name) + r'\b'
                    clickable_line = re.sub(pattern, link, clickable_line)
                    
            clickable_imports.append({
                'line': import_info['line'],
                'clickable_line': clickable_line,
                'modules': import_info['modules']
            })
            
        return clickable_imports
    
    def _build_inheritance_chains(self):
        """Build inheritance chains and used_by relationships after all modules are loaded."""
        # Create lookup map for faster access
        module_lookup = {m['name']: m for m in self.modules}
        
        for module in self.modules:
            if parent_name := module.get('parent_module'):
                # Build inheritance chain
                chain = []
                current = parent_name
                visited = set()
                
                while current and current not in visited:
                    visited.add(current)
                    if parent_module := module_lookup.get(current):
                        chain.append({
                            'name': parent_module['name'],
                            'type': parent_module['type'],
                            'icon': parent_module['icon'],
                            'path': parent_module.get('path', ''),
                            'file_path': parent_module.get('file_path', '')
                        })
                        current = parent_module.get('parent_module')
                    else:
                        # Parent not found in current project, add as external
                        chain.append({
                            'name': current,
                            'type': 'External',
                            'icon': '📦',
                            'path': '',
                            'file_path': ''
                        })
                        break
                
                module['inheritance_chain'] = chain
                
                # Update used_by relationships
                if parent_name in module_lookup:
                    parent_module = module_lookup[parent_name]
                    if 'used_by' not in parent_module:
                        parent_module['used_by'] = []
                    parent_module['used_by'].append({
                        'name': module['name'],
                        'type': module['type'],
                        'icon': module['icon'],
                        'path': module.get('path', ''),
                        'file_path': module.get('file_path', '')
                    })
    
    def _organize_modules(self) -> Dict[str, List[Dict]]:
        """Organize modules into categories for better navigation."""
        organized = {
            'project': [],           # Main project modules
            'dependencies': [],      # External package modules  
            'standard_library': [], # Built-in atopile modules
            'parts': []             # Hardware part packages
        }
        
        for module in self.modules:
            file_path = module['file_path']
            
            # Categorize based on file path patterns
            if file_path.startswith('.ato/modules/'):
                # External dependency
                organized['dependencies'].append(module)
            elif any(keyword in file_path.lower() for keyword in ['part', 'package']):
                # Hardware part
                organized['parts'].append(module)
            elif file_path.count('/') <= 1 and not file_path.startswith('.'):
                # Project module (in root or one level deep)
                organized['project'].append(module)
            elif any(std_name in file_path for std_name in ['common/', 'interfaces/', 'debug/']):
                # Standard library modules
                organized['standard_library'].append(module)
            else:
                # Default to dependencies
                organized['dependencies'].append(module)
        
        # Sort each category
        for category in organized.values():
            category.sort(key=lambda x: x['name'])
            
        return organized
        
    def _generate_index(self):
        """Generate the index page."""
        template = self.env.get_template('index.html')
        
        # Organize modules into categories
        organized_modules = self._organize_modules()
        
        # Calculate statistics
        stats = {
            'total_modules': sum(1 for m in self.modules if m['type'].lower() == 'module'),
            'total_interfaces': sum(1 for m in self.modules if m['type'].lower() == 'interface'),
            'total_components': sum(1 for m in self.modules if m['type'].lower() == 'component'),
            'total_files': len(self.files)
        }
        
        html = template.render(
            project_name=self.project_path.name,
            modules=organized_modules['project'],
            organized_modules=organized_modules,
            all_modules=sorted(self.modules, key=lambda x: x['name']),
            files=sorted(self.files, key=lambda x: x['path']),
            stats=stats
        )
        
        (self.output_dir / 'index.html').write_text(html)
        
    def _generate_module_pages(self):
        """Generate individual module pages."""
        template = self.env.get_template('module.html')
        module_dir = self.output_dir / 'module'
        
        # Get organized modules for consistent navigation
        organized_modules = self._organize_modules()
        
        for module in self.modules:
            # Create module directory structure
            module_path = module_dir / module['path']
            module_path.parent.mkdir(parents=True, exist_ok=True)
            
            html = template.render(
                project_name=self.project_path.name,
                module=module,
                modules=organized_modules['project'],
                organized_modules=organized_modules,
                current_module=module['path']
            )
            
            (module_path.with_suffix('.html')).write_text(html)